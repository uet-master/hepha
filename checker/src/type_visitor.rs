// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fmt::{Debug, Formatter, Result};
use std::ops::DerefMut;
use std::rc::Rc;

use log_derive::*;

use hepha_annotations::*;
use rustc_hir::def_id::DefId;
use rustc_index::Idx;
use rustc_middle::mir;
use rustc_middle::ty::{
    AdtDef, Const, ConstKind, CoroutineArgsExt, ExistentialPredicate, ExistentialProjection,
    ExistentialTraitRef, FnSig, GenericArg, GenericArgKind, GenericArgs, GenericArgsRef, ParamTy,
    Ty, TyCtxt, TyKind,
};
use rustc_target::abi::VariantIdx;

use crate::abstract_value::AbstractValue;
use crate::constant_domain::ConstantDomain;
use crate::environment::Environment;
use crate::expression::{Expression, ExpressionType};
use crate::path::{Path, PathEnum, PathRefinement, PathRoot, PathSelector};
use crate::{type_visitor, utils};

#[derive(Debug)]
pub struct TypeCache<'tcx> {
    type_list: Vec<Ty<'tcx>>,
    type_to_index_map: HashMap<Ty<'tcx>, usize>,
}

impl Default for type_visitor::TypeCache<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'tcx> TypeCache<'tcx> {
    /// Provides a way to refer to a rustc_middle::ty::Ty via a handle that does not have
    /// a life time specifier.
    pub fn new() -> TypeCache<'tcx> {
        TypeCache {
            type_list: Vec::with_capacity(10_000),
            type_to_index_map: HashMap::with_capacity(10_000),
        }
    }

    /// Returns a non zero index that can be used to retrieve ty via get_type.
    pub fn get_index(&mut self, ty: &Ty<'tcx>) -> usize {
        if let Some(index) = self.type_to_index_map.get(ty) {
            *index
        } else {
            let index = self.type_list.len() + 1;
            self.type_list.push(*ty);
            self.type_to_index_map.insert(*ty, index);
            index
        }
    }

    /// Returns the type that was stored at this index, or None if index is zero
    /// or greater than the length of the type list.
    pub fn get_type(&self, index: usize) -> Option<Ty<'tcx>> {
        if index == 0 {
            return None;
        }
        self.type_list.get(index - 1).cloned()
    }
}

#[derive(Clone)]
pub struct TypeVisitor<'tcx> {
    pub actual_argument_types: Vec<Ty<'tcx>>,
    pub closures_being_specialized: RefCell<HashSet<DefId>>,
    pub def_id: DefId,
    pub generic_argument_map: Option<HashMap<rustc_span::Symbol, GenericArg<'tcx>>>,
    pub generic_arguments: Option<GenericArgsRef<'tcx>>,
    pub mir: &'tcx mir::Body<'tcx>,
    path_ty_cache: HashMap<Rc<Path>, Ty<'tcx>>,
    pub dummy_untagged_value_type: Ty<'tcx>,
    tcx: TyCtxt<'tcx>,
    type_cache: Rc<RefCell<TypeCache<'tcx>>>,
}

impl Debug for TypeVisitor<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        "TypeVisitor".fmt(f)
    }
}

impl<'tcx> TypeVisitor<'tcx> {
    pub fn new(
        def_id: DefId,
        mir: &'tcx mir::Body<'tcx>,
        tcx: TyCtxt<'tcx>,
        cache: Rc<RefCell<TypeCache<'tcx>>>,
    ) -> TypeVisitor<'tcx> {
        let dummy_untagged_value_type = tcx.types.i8;
        TypeVisitor {
            actual_argument_types: Vec::new(),
            closures_being_specialized: RefCell::new(HashSet::new()),
            def_id,
            generic_argument_map: None,
            generic_arguments: None,
            mir,
            path_ty_cache: HashMap::new(),
            dummy_untagged_value_type,
            tcx,
            type_cache: cache,
        }
    }

    /// Restores the method only state to its initial state.
    #[logfn_inputs(TRACE)]
    pub fn reset_visitor_state(&mut self) {
        self.generic_arguments = None;
        self.path_ty_cache
            .retain(|p, _| p.is_rooted_by_non_local_structure());
    }

    /// Parameters that are closures bring enclosed variables with them that are effectively
    /// additional parameters. We pre-populate the environment with entries for these because
    /// there is no convenient way to look up their types later on. I.e. unlike ordinary parameters
    /// whose types can be looked up in mir.local_decls, these extra parameters need their
    /// types extracted from the closure type definitions via the tricky logic below.
    #[logfn_inputs(TRACE)]
    pub fn add_any_closure_fields_for(
        &mut self,
        mut path_ty: &Ty<'tcx>,
        path: &Rc<Path>,
        first_state: &mut Environment,
    ) {
        let mut is_ref = false;
        if let TyKind::Ref(_, t, _) = path_ty.kind() {
            is_ref = true;
            path_ty = t;
        }
        match path_ty.kind() {
            TyKind::Closure(_, args) => {
                if utils::are_concrete(args) {
                    for (i, ty) in args.as_closure().upvar_tys().iter().enumerate() {
                        let var_type = ExpressionType::from(ty.kind());
                        let mut qualifier = path.clone();
                        if is_ref {
                            qualifier = Path::new_deref(path.clone(), ExpressionType::NonPrimitive)
                        }
                        let closure_field_path = Path::new_field(qualifier, i);
                        self.set_path_rustc_type(closure_field_path.clone(), ty);
                        let closure_field_val =
                            AbstractValue::make_typed_unknown(var_type, closure_field_path.clone());
                        first_state
                            .value_map
                            .insert_mut(closure_field_path, closure_field_val);
                    }
                }
            }
            TyKind::Coroutine(_, args) => {
                for (i, ty) in args.as_coroutine().prefix_tys().iter().enumerate() {
                    let var_type = ExpressionType::from(ty.kind());
                    let mut qualifier = path.clone();
                    if is_ref {
                        qualifier = Path::new_deref(path.clone(), ExpressionType::NonPrimitive)
                    }
                    let generator_field_path = Path::new_field(qualifier, i);
                    self.set_path_rustc_type(generator_field_path.clone(), ty);
                    let generator_field_val =
                        AbstractValue::make_typed_unknown(var_type, generator_field_path.clone());
                    first_state
                        .value_map
                        .insert_mut(generator_field_path, generator_field_val);
                }
            }
            TyKind::Alias(
                rustc_middle::ty::Opaque,
                rustc_middle::ty::AliasTy { def_id, args, .. },
            ) => {
                let map = self.get_generic_arguments_map(*def_id, args, &[]);
                let path_ty = self.specialize_type(self.tcx.type_of(*def_id).skip_binder(), &map);
                self.add_any_closure_fields_for(&path_ty, path, first_state);
            }
            TyKind::Dynamic(..) | TyKind::FnDef(..) | TyKind::FnPtr(..) => {}
            _ => {
                info!("unexpected closure type {:?}", path_ty.kind());
            }
        }
    }

    /// Returns the size in bytes (including padding) of an element of the given collection type.
    /// If the type is not a collection, it returns one.
    pub fn get_elem_type_size(&self, ty: Ty<'tcx>) -> u64 {
        match ty.kind() {
            TyKind::Array(ty, _) | TyKind::Slice(ty) => self.get_type_size(*ty),
            TyKind::RawPtr(ty, _) => self.get_type_size(*ty),
            _ => 1,
        }
    }

    /// Path is required to be rooted in a temporary used to track a checked operation result.
    /// The result type of the local will be a tuple (t, bool).
    /// The result of this function is the t part.
    #[logfn_inputs(TRACE)]
    pub fn get_first_part_of_target_path_type_tuple(
        &self,
        path: &Rc<Path>,
        current_span: rustc_span::Span,
    ) -> ExpressionType {
        match self.get_path_rustc_type(path, current_span).kind() {
            TyKind::Tuple(types) => ExpressionType::from(types[0].kind()),
            _ => assume_unreachable!(),
        }
    }

    // Path is required to be rooted in a temporary used to track an operation result.
    #[logfn_inputs(TRACE)]
    pub fn get_target_path_type(
        &self,
        path: &Rc<Path>,
        current_span: rustc_span::Span,
    ) -> ExpressionType {
        ExpressionType::from(self.get_path_rustc_type(path, current_span).kind())
    }

    /// Returns a parameter environment for the current function.
    pub fn get_param_env(&self) -> rustc_middle::ty::ParamEnv<'tcx> {
        let env_def_id = if self.tcx.is_closure_like(self.def_id) {
            self.tcx.typeck_root_def_id(self.def_id)
        } else {
            self.def_id
        };
        self.tcx.param_env(env_def_id)
    }

    pub fn get_typing_env(&self) -> rustc_middle::ty::TypingEnv<'tcx> {
        let param_env = self.get_param_env();
        rustc_middle::ty::TypingEnv {
            typing_mode: rustc_middle::ty::TypingMode::PostAnalysis,
            param_env,
        }
    }

    pub fn get_typing_env_for(&self, def_id: DefId) -> rustc_middle::ty::TypingEnv<'tcx> {
        let env_def_id = if self.tcx.is_closure_like(def_id) {
            self.tcx.typeck_root_def_id(def_id)
        } else {
            def_id
        };
        let param_env = self.tcx.param_env(env_def_id);
        rustc_middle::ty::TypingEnv {
            typing_mode: rustc_middle::ty::TypingMode::PostAnalysis,
            param_env,
        }
    }

    /// Returns a shared reference to the path type cache of the visitor
    pub fn get_path_type_cache(&self) -> &HashMap<Rc<Path>, Ty<'tcx>> {
        &self.path_ty_cache
    }

    pub fn get_index_for(&self, ty: Ty<'tcx>) -> usize {
        let mut cache = self.type_cache.borrow_mut();
        cache.get_index(&ty)
    }

    pub fn get_type_from_index(&self, type_index: usize) -> Ty<'tcx> {
        let cache = self.type_cache.borrow();
        if let Some(ty) = cache.get_type(type_index) {
            ty
        } else {
            self.tcx.types.never
        }
    }

    pub fn is_empty_struct(&self, ty_kind: &TyKind<'tcx>) -> bool {
        if let TyKind::Adt(def, ..) = ty_kind {
            if !def.is_enum() && !def.is_union() {
                let variant = &def.variants()[VariantIdx::new(0)];
                return variant.fields.is_empty();
            }
        }
        false
    }

    #[logfn_inputs(TRACE)]
    pub fn is_function_like(&self, ty_kind: &TyKind<'tcx>) -> bool {
        matches!(
            ty_kind,
            TyKind::Closure(..)
                | TyKind::Dynamic(..)
                | TyKind::FnDef(..)
                | TyKind::FnPtr(_, _)
                | TyKind::Foreign(..)
                | TyKind::Coroutine(..)
                | TyKind::CoroutineWitness(..)
                | TyKind::Alias(rustc_middle::ty::Opaque, ..)
        )
    }

    /// Returns true if the given type is a reference (or raw pointer) to a collection type, in which
    /// case the reference/pointer independently tracks the length of the collection, thus effectively
    /// tracking a slice of the underlying collection.
    #[logfn_inputs(TRACE)]
    pub fn is_slice_pointer(&self, ty_kind: &TyKind<'tcx>) -> bool {
        match ty_kind {
            TyKind::RawPtr(target, _) | TyKind::Ref(_, target, _) => {
                trace!("target type {:?}", target.kind());
                // Pointers to sized arrays are thin pointers.
                matches!(target.kind(), TyKind::Slice(..) | TyKind::Str)
            }
            _ => false,
        }
    }

    /// Returns true if the given type is a reference to the string type.
    #[logfn_inputs(TRACE)]
    pub fn is_string_pointer(&self, ty_kind: &TyKind<'tcx>) -> bool {
        if let TyKind::Ref(_, target, _) = ty_kind {
            matches!(target.kind(), TyKind::Str)
        } else {
            false
        }
    }

    /// Returns true if the given type is a reference (or raw pointer) that is not a slice pointer
    #[logfn_inputs(TRACE)]
    pub fn is_thin_pointer(&self, ty_kind: &TyKind<'tcx>) -> bool {
        match ty_kind {
            TyKind::RawPtr(target, _) | TyKind::Ref(_, target, _) => {
                !matches!(target.kind(), TyKind::Slice(..) | TyKind::Str)
            }
            _ => false,
        }
    }

    /// Updates the type cache of the visitor so that looking up the type of path returns ty.
    #[logfn_inputs(TRACE)]
    pub fn set_path_rustc_type(&mut self, path: Rc<Path>, ty: Ty<'tcx>) {
        self.path_ty_cache.insert(path, ty);
    }

    /// This is a hacky and brittle way to navigate the Rust compiler's type system.
    /// Eventually it should be replaced with a comprehensive and principled mapping.
    #[logfn_inputs(TRACE)]
    pub fn get_path_rustc_type(&self, path: &Rc<Path>, current_span: rustc_span::Span) -> Ty<'tcx> {
        if let Some(ty) = self.path_ty_cache.get(path) {
            return *ty;
        }
        match &path.value {
            PathEnum::Computed { value } => match &value.expression {
                Expression::ConditionalExpression { consequent: e, .. }
                | Expression::Join { left: e, .. } => {
                    self.get_path_rustc_type(&Path::get_as_path(e.clone()), current_span)
                }
                Expression::CompileTimeConstant(c) => {
                    if let ConstantDomain::Function(fr) = c {
                        if let Some(def_id) = fr.def_id {
                            return self.tcx.type_of(def_id).skip_binder();
                        }
                    }
                    c.get_rustc_type(self.tcx)
                }
                Expression::Reference(path) => {
                    let target_type = self.get_path_rustc_type(path, current_span);
                    if target_type.is_never() {
                        target_type
                    } else {
                        Ty::new_imm_ref(self.tcx, self.tcx.lifetimes.re_erased, target_type)
                    }
                }
                Expression::InitialParameterValue { path, .. }
                | Expression::Variable { path, .. } => self.get_path_rustc_type(path, current_span),
                _ => value.expression.infer_type().as_rustc_type(self.tcx),
            },
            PathEnum::LocalVariable {
                ordinal,
                type_index,
            } => {
                if *ordinal > 0 && *ordinal < self.mir.local_decls.len() {
                    let t = self.get_type_from_index(*type_index);
                    if t.is_never() {
                        self.get_loc_ty(mir::Local::from(*ordinal))
                    } else {
                        t
                    }
                } else {
                    trace!(
                        "local var path.value is {:?} at {:?}",
                        path.value,
                        current_span
                    );
                    self.get_type_from_index(*type_index)
                }
            }
            PathEnum::HeapBlock { .. } => {
                Ty::new_ptr(self.tcx, self.tcx.types.u8, rustc_hir::Mutability::Not)
            }
            PathEnum::Offset { value } => {
                if let Expression::Offset { left, .. } = &value.expression {
                    let base_path = Path::get_as_path(left.clone());
                    self.get_path_rustc_type(&base_path, current_span)
                } else {
                    unreachable!("an offset path, must contain an offset expression");
                }
            }
            PathEnum::Parameter { ordinal } => {
                if *ordinal > 0 && *ordinal < self.mir.local_decls.len() {
                    self.get_loc_ty(mir::Local::from(*ordinal))
                } else {
                    info!(
                        "parameter path.value is {:?} at {:?}",
                        path.value, current_span
                    );
                    self.tcx.types.never
                }
            }
            PathEnum::PhantomData => self.tcx.types.never,
            PathEnum::PromotedConstant { .. } => {
                debug!("path.value is {:?} at {:?}", path.value, current_span);
                debug!("path_ty_cache {:?}", self.path_ty_cache);
                self.tcx.types.never
            }
            PathEnum::Result => {
                if self.mir.local_decls.is_empty() {
                    info!("result type wanted from function without result local");
                    self.tcx.types.never
                } else {
                    self.specialize_type(
                        self.mir.local_decls[mir::Local::from(0usize)].ty,
                        &self.generic_argument_map,
                    )
                }
            }
            PathEnum::QualifiedPath {
                qualifier,
                selector,
                ..
            } => {
                let mut t = self.get_path_rustc_type(qualifier, current_span);
                if t.is_never() {
                    return t;
                }
                match t.kind() {
                    TyKind::Infer(..) => {
                        // The qualifier does not resolve to a useful rustc type.
                        // This can happen when the qualifier is a PathEnum::Computed where the value
                        // is TOP, or BOTTOM or a heap layout.
                        return self.tcx.types.never;
                    }
                    TyKind::Alias(rustc_middle::ty::Projection, ..) => {
                        t = self.specialize_type(t, &self.generic_argument_map);
                    }
                    _ => {}
                }
                match &**selector {
                    PathSelector::ConstantSlice { .. } => {
                        return Ty::new_imm_ref(self.tcx, self.tcx.lifetimes.re_erased, t);
                    }
                    PathSelector::Function => {
                        return t;
                    }
                    PathSelector::UnionField {
                        case_index: ordinal,
                        ..
                    }
                    | PathSelector::Field(ordinal) => {
                        if let TyKind::Alias(
                            rustc_middle::ty::Opaque,
                            rustc_middle::ty::AliasTy { def_id, args, .. },
                        ) = &t.kind()
                        {
                            let map = self.get_generic_arguments_map(*def_id, args, &[]);
                            t = self.specialize_type(self.tcx.type_of(*def_id).skip_binder(), &map);
                            trace!("opaque type_of {:?}", t.kind());
                            trace!("opaque type_of {:?}", t);
                        }
                        match t.kind() {
                            TyKind::Adt(def, args) => {
                                return self.get_field_type(def, args, *ordinal);
                            }
                            TyKind::Array(elem_ty, ..) | TyKind::Slice(elem_ty) => {
                                match *ordinal {
                                    0 => {
                                        // Field 0 of a sized array is a raw pointer to the array element type
                                        return Ty::new_ptr(
                                            self.tcx,
                                            *elem_ty,
                                            rustc_hir::Mutability::Not,
                                        );
                                    }
                                    1 => {
                                        return self.tcx.types.usize;
                                    }
                                    _ => {}
                                }
                            }
                            TyKind::Closure(def_id, args) => {
                                let closure_substs = args.as_closure();
                                return *closure_substs.upvar_tys().get(*ordinal).unwrap_or_else(
                                    || {
                                        info!("closure field not found {:?} {:?}", def_id, ordinal);
                                        &self.tcx.types.never
                                    },
                                );
                            }
                            TyKind::Coroutine(def_id, args) => {
                                let mut tuple_types =
                                    args.as_coroutine().state_tys(*def_id, self.tcx);
                                if let Some(field_tys) = tuple_types.nth(*ordinal) {
                                    return Ty::new_tup_from_iter(self.tcx, field_tys);
                                }
                                info!("generator field not found {:?} {:?}", def_id, ordinal);
                                return self.tcx.types.never;
                            }
                            TyKind::Ref(_, t, _) if matches!(t.kind(), TyKind::Closure(..)) => {
                                // todo: this seems to work around a more fundamental bug.
                                // why would getting a field from a closure not need a deref
                                // before the field access? I.e. is a reference to a closure
                                // a sort of fat pointer?
                                if let TyKind::Closure(def_id, args) = t.kind() {
                                    if utils::are_concrete(args) {
                                        return *args
                                            .as_closure()
                                            .upvar_tys()
                                            .get(*ordinal)
                                            .unwrap_or_else(|| {
                                                info!(
                                                    "closure field not found {:?} {:?}",
                                                    def_id, ordinal
                                                );
                                                &self.tcx.types.never
                                            });
                                    }
                                } else {
                                    unreachable!("t.kind is a closure because of the guard");
                                }
                            }
                            TyKind::Str => {
                                match *ordinal {
                                    0 => {
                                        // Field 0 of a str is a raw pointer to char
                                        return Ty::new_ptr(
                                            self.tcx,
                                            self.tcx.types.char,
                                            rustc_hir::Mutability::Not,
                                        );
                                    }
                                    1 => {
                                        return self.tcx.types.usize;
                                    }
                                    _ => {}
                                }
                            }
                            TyKind::Tuple(types) => {
                                if let Some(ty) = types.get(*ordinal) {
                                    return *ty;
                                }
                                if types.is_empty() {
                                    return self.tcx.types.never;
                                }
                            }
                            _ => {
                                if self.is_slice_pointer(t.kind()) {
                                    match *ordinal {
                                        0 => {
                                            // Field 0 of a slice pointer is a raw pointer to the slice element type
                                            return Ty::new_ptr(
                                                self.tcx,
                                                self.get_element_type(t),
                                                rustc_hir::Mutability::Mut,
                                            );
                                        }
                                        1 => {
                                            return self.tcx.types.usize;
                                        }
                                        _ => {}
                                    }
                                } else {
                                    // Taking the address of a struct returns the address of field 0
                                    // and the type of the address is both &S and &F where S is the
                                    // struct type and F is the type of field 0. If get here, it
                                    // is because we tracked type F, whereas rustc used S.
                                    match *ordinal {
                                        0 => {
                                            return t;
                                        }
                                        1 => {
                                            // Assume &S is a slice pointer
                                            return self.tcx.types.usize;
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                    PathSelector::Deref => {
                        return self.get_dereferenced_type(t);
                    }
                    PathSelector::Discriminant => {
                        return self.tcx.types.i32;
                    }
                    PathSelector::Downcast(_, ordinal, _) => {
                        // Down casting to an enum variant
                        if t == self.tcx.types.usize {
                            // Down casting from an untyped pointer. This happens often enough
                            // that we don't want to log this an informational message.
                            debug!("The qualifier of the downcast can't be typed");
                            return self.tcx.types.never;
                        }
                        while type_visitor::is_transparent_wrapper(t)
                            || matches!(t.kind(), TyKind::Adt(..))
                        {
                            if let TyKind::Adt(def, args) = t.kind() {
                                let args =
                                    self.specialize_generic_args(args, &self.generic_argument_map);
                                if !def.is_enum() {
                                    // Could be a *&S vs *&S.Field_0 confusion
                                    t = self.get_field_type(def, args, 0);
                                    continue;
                                }
                                if *ordinal < def.variants().len() {
                                    let variant = &def.variants()[VariantIdx::new(*ordinal)];
                                    let field_tys =
                                        variant.fields.iter().map(|fd| fd.ty(self.tcx, args));
                                    return Ty::new_tup_from_iter(self.tcx, field_tys);
                                }
                                if !type_visitor::is_transparent_wrapper(t) {
                                    break;
                                }
                            }
                            t = self.remove_transparent_wrapper(t);
                        }
                        info!(
                            "illegally down casting to index {} of {:?} at {:?}",
                            *ordinal, t, current_span
                        );
                        return self.tcx.types.never;
                    }
                    PathSelector::Index(_) | PathSelector::ConstantIndex { .. } => {
                        return self.get_element_type(t);
                    }
                    PathSelector::Layout => {
                        return self.tcx.types.trait_object_dummy_self;
                    }
                    PathSelector::Slice(_) => {
                        return {
                            let slice_ty = Ty::new_slice(self.tcx, self.get_element_type(t));
                            Ty::new_mut_ref(self.tcx, self.tcx.lifetimes.re_static, slice_ty)
                        };
                    }
                    PathSelector::TagField => {
                        return self.dummy_untagged_value_type;
                    }
                    _ => {}
                }
                info!("current span is {:?}", current_span);
                info!(
                    "cache key is {:?}",
                    utils::summary_key_str(self.tcx, self.def_id)
                );
                info!("path is {:?}", path);
                info!("t is {:?}", t);
                info!("qualifier is {:?}", qualifier);
                info!("selector is {:?}", selector);
                self.tcx.types.never
            }
            PathEnum::StaticVariable { def_id, .. } => {
                if let Some(def_id) = def_id {
                    return self.tcx.type_of(*def_id).skip_binder();
                }
                info!(
                    "static variable path.value is {:?} at {:?}",
                    path.value, current_span
                );
                self.tcx.types.never
            }
        }
    }

    /// Returns the target type of a reference type.
    #[logfn_inputs(TRACE)]
    pub fn get_dereferenced_type(&self, ty: Ty<'tcx>) -> Ty<'tcx> {
        match ty.kind() {
            TyKind::RawPtr(ty, _) => *ty,
            TyKind::Ref(_, t, _) => *t,
            _ => {
                if ty.is_box() {
                    ty.boxed_ty().expect("ty.is_box()")
                } else {
                    ty
                }
            }
        }
    }

    /// Returns the element type of an array or slice type.
    #[logfn_inputs(TRACE)]
    pub fn get_element_type(&self, ty: Ty<'tcx>) -> Ty<'tcx> {
        match &ty.kind() {
            TyKind::Array(t, _) => *t,
            TyKind::RawPtr(t, _) | TyKind::Ref(_, t, _) => match t.kind() {
                TyKind::Array(t, _) => *t,
                TyKind::Slice(t) => *t,
                TyKind::Str => self.tcx.types.char,
                _ => *t,
            },
            TyKind::Slice(t) => *t,
            TyKind::Str => self.tcx.types.char,
            _ => ty,
        }
    }

    /// Returns the type of the field with the given ordinal.
    #[logfn_inputs(TRACE)]
    pub fn get_field_type(
        &self,
        def: &'tcx AdtDef,
        args: GenericArgsRef<'tcx>,
        ordinal: usize,
    ) -> Ty<'tcx> {
        for variant in def.variants().iter() {
            if ordinal < variant.fields.len() {
                let field = &variant.fields[ordinal.into()];
                let ft = field.ty(self.tcx, args);
                trace!("field {:?} type is {:?}", ordinal, ft);
                return ft;
            }
        }
        debug!("adt def does not have a field with ordinal {}", ordinal);
        self.tcx.types.never
    }

    /// Returns a map from path to ADT type for any path rooted in an actual argument
    /// and known to have a type that is a reference to an ADT. Since the rustc type of the
    /// corresponding field might be a trait, we prefer to type from the actual argument which
    /// is more likely to be concrete. By seeding the initial type cache of a called function
    /// with this information, we can get resolution of trait calls where the receiver is a
    /// field reachable from a parameter, rather than the parameter itself.
    #[logfn_inputs(TRACE)]
    pub fn get_adt_map(
        &self,
        actual_arguments: &[(Rc<Path>, Rc<AbstractValue>)],
        environment: &Environment,
    ) -> Option<Rc<HashMap<Rc<Path>, Ty<'tcx>>>> {
        let mut result: HashMap<Rc<Path>, Ty<'tcx>> = HashMap::new();
        for (i, (arg_path, _)) in actual_arguments.iter().enumerate() {
            for (p, v) in environment
                .value_map
                .iter()
                .filter(|(p, _)| p.is_rooted_by(arg_path))
            {
                if let Expression::Reference(rp) = &v.expression {
                    if let Some(ty) = self.path_ty_cache.get(rp) {
                        if ty.is_adt() {
                            let param_path = p.replace_root(arg_path, Path::new_parameter(i + 1));
                            let ptr_ty = Ty::new_ptr(self.tcx, *ty, rustc_hir::Mutability::Not);
                            result.insert(param_path, ptr_ty);
                        }
                    }
                }
            }
        }
        if result.is_empty() {
            None
        } else {
            Some(Rc::new(result))
        }
    }

    /// If Operand corresponds to a compile time constant function, return
    /// the generic parameter substitutions (type arguments) that are used by
    /// the call instruction whose operand this is.
    #[logfn_inputs(TRACE)]
    pub fn get_generic_arguments_map(
        &self,
        def_id: DefId,
        generic_args: GenericArgsRef<'tcx>,
        actual_argument_types: &[Ty<'tcx>],
    ) -> Option<HashMap<rustc_span::Symbol, GenericArg<'tcx>>> {
        let mut substitution_map = self.generic_argument_map.clone();
        let mut map: HashMap<rustc_span::Symbol, GenericArg<'tcx>> = HashMap::new();

        // This iterates over the callee's generic parameter definitions.
        // If the parent of the callee is generic, those definitions are iterated
        // as well. This applies recursively. Note that a child cannot mask the
        // generic parameters of its parent with one of its own, so each parameter
        // definition in this iteration will have a unique name.
        GenericArgs::for_item(self.tcx, def_id, |param_def, _| {
            if let Some(gen_arg) = generic_args.get(param_def.index as usize) {
                let specialized_gen_arg =
                    self.specialize_generic_argument(*gen_arg, &substitution_map);
                if let Some(substitution_map) = &mut substitution_map {
                    substitution_map.insert(param_def.name, specialized_gen_arg);
                }
                map.insert(param_def.name, specialized_gen_arg);
            } else {
                debug!("unmapped generic param def {:?}", param_def);
            }
            self.tcx.mk_param_from_def(param_def) // not used
        });
        // Add "Self" -> actual_argument_types[0]
        if let Some(self_ty) = actual_argument_types.first() {
            let self_ty = if let TyKind::Ref(_, ty, _) = self_ty.kind() {
                *ty
            } else {
                *self_ty
            };
            let self_sym = rustc_span::Symbol::intern("Self");
            map.entry(self_sym).or_insert_with(|| self_ty.into());
        }
        if map.is_empty() {
            None
        } else {
            Some(map)
        }
    }

    /// Returns the specialized type for the given local variable
    #[logfn_inputs(TRACE)]
    pub fn get_loc_ty(&self, local: mir::Local) -> Ty<'tcx> {
        let i = local.as_usize();
        let loc_ty =
            self.specialize_type(self.mir.local_decls[local].ty, &self.generic_argument_map);
        if !utils::is_concrete(loc_ty.kind())
            && 0 < i
            && i <= self.mir.arg_count
            && i <= self.actual_argument_types.len()
        {
            let act_ty = self.actual_argument_types[i - 1];
            if utils::is_concrete(act_ty.kind()) {
                return act_ty;
            }
        }
        loc_ty
    }

    /// Returns an ExpressionType value corresponding to the Rustc type of the place.
    #[logfn_inputs(TRACE)]
    pub fn get_place_type(
        &self,
        place: &mir::Place<'tcx>,
        current_span: rustc_span::Span,
    ) -> ExpressionType {
        ExpressionType::from(self.get_rustc_place_type(place, current_span).kind())
    }

    /// Returns the rustc Ty of the given place in memory.
    #[logfn_inputs(TRACE)]
    #[logfn(TRACE)]
    pub fn get_rustc_place_type(
        &self,
        place: &mir::Place<'tcx>,
        current_span: rustc_span::Span,
    ) -> Ty<'tcx> {
        let result = {
            let base_type = self.get_loc_ty(place.local);
            self.get_type_for_projection_element(current_span, base_type, place.projection)
        };
        match result.kind() {
            TyKind::Param(t_par) => {
                if let Some(generic_args) = self.generic_arguments {
                    if let Some(ty) = generic_args.types().nth(t_par.index as usize) {
                        return ty;
                    }
                    if t_par.name.as_str() == "Self" && !self.actual_argument_types.is_empty() {
                        return self.actual_argument_types[0];
                    }
                }
            }
            TyKind::Ref(region, ty, mutbl) => {
                if let TyKind::Param(t_par) = ty.kind() {
                    if t_par.name.as_str() == "Self" && !self.actual_argument_types.is_empty() {
                        return Ty::new_ref(
                            self.tcx,
                            *region,
                            self.actual_argument_types[0],
                            *mutbl,
                        );
                    }
                }
            }
            _ => {}
        }
        result
    }

    /// Returns the rustc TyKind of the element selected by projection_elem.
    #[logfn_inputs(TRACE)]
    pub fn get_type_for_projection_element(
        &self,
        current_span: rustc_span::Span,
        base_ty: Ty<'tcx>,
        place_projection: &[rustc_middle::mir::PlaceElem<'tcx>],
    ) -> Ty<'tcx> {
        place_projection
            .iter()
            .fold(base_ty, |base_ty, projection_elem| match projection_elem {
                mir::ProjectionElem::Deref => match base_ty.kind() {
                    TyKind::Adt(..) => base_ty,
                    TyKind::RawPtr(ty, _) => *ty,
                    TyKind::Ref(_, ty, _) => *ty,
                    _ => {
                        info!(
                            "bad deref projection span: {:?}\nelem: {:?} type: {:?}",
                            current_span, projection_elem, base_ty
                        );
                        self.tcx.types.never
                    }
                },
                mir::ProjectionElem::Field(_, ty) => {
                    self.specialize_type(*ty, &self.generic_argument_map)
                }
                mir::ProjectionElem::Subslice { .. } => base_ty,
                mir::ProjectionElem::Index(_) | mir::ProjectionElem::ConstantIndex { .. } => {
                    match base_ty.kind() {
                        TyKind::Adt(..) => base_ty,
                        TyKind::Array(ty, _) => *ty,
                        TyKind::Ref(_, ty, _) => self.get_element_type(*ty),
                        TyKind::Slice(ty) => *ty,
                        _ => {
                            debug!(
                                "span: {:?}\nelem: {:?} type: {:?}",
                                current_span, projection_elem, base_ty
                            );
                            assume_unreachable!();
                        }
                    }
                }
                mir::ProjectionElem::Subtype(ty) => *ty,
                mir::ProjectionElem::OpaqueCast(ty) => *ty,
                mir::ProjectionElem::Downcast(_, ordinal) => {
                    if let TyKind::Adt(def, args) = base_ty.kind() {
                        if ordinal.index() >= def.variants().len() {
                            debug!(
                                "illegally down casting to index {} of {:?} at {:?}",
                                ordinal.index(),
                                base_ty,
                                current_span
                            );
                            let variant = &def.variants().iter().last().unwrap();
                            let field_tys = variant.fields.iter().map(|fd| fd.ty(self.tcx, args));
                            return Ty::new_tup_from_iter(self.tcx, field_tys);
                        }
                        let variant = &def.variants()[*ordinal];
                        let field_tys = variant.fields.iter().map(|fd| fd.ty(self.tcx, args));
                        return Ty::new_tup_from_iter(self.tcx, field_tys);
                    } else if let TyKind::Coroutine(def_id, args) = base_ty.kind() {
                        let mut tuple_types = args.as_coroutine().state_tys(*def_id, self.tcx);
                        if let Some(field_tys) = tuple_types.nth(ordinal.index()) {
                            return Ty::new_tup_from_iter(self.tcx, field_tys);
                        }
                        debug!(
                            "illegally down casting to index {} of {:?} at {:?}",
                            ordinal.index(),
                            base_ty,
                            current_span
                        );
                    } else {
                        info!("unexpected type for downcast {:?}", base_ty);
                    }
                    base_ty
                }
            })
    }

    /// Returns the size in bytes (including padding) of an instance of the given type.
    pub fn get_type_size(&self, ty: Ty<'tcx>) -> u64 {
        if let Ok(ty_and_layout) = self.layout_of(ty) {
            ty_and_layout.layout.size().bytes()
        } else {
            0
        }
    }

    /// Returns the size (including padding) and alignment, in bytes,  of an instance of the given type.
    pub fn get_type_size_and_alignment(&self, ty: Ty<'tcx>) -> (u128, u128) {
        if let Ok(ty_and_layout) = self.layout_of(ty) {
            (
                ty_and_layout.layout.size().bytes() as u128,
                ty_and_layout.align.pref.bytes() as u128,
            )
        } else {
            (0, 8)
        }
    }

    /// Returns a layout for the given type, if concrete.
    pub fn layout_of(
        &self,
        ty: Ty<'tcx>,
    ) -> std::result::Result<
        rustc_middle::ty::layout::TyAndLayout<'tcx>,
        &'tcx rustc_middle::ty::layout::LayoutError<'tcx>,
    > {
        let typing_env = self.get_typing_env();
        if utils::is_concrete(ty.kind()) {
            self.tcx.layout_of(typing_env.as_query_input(ty))
        } else {
            Err(&*self
                .tcx
                .arena
                .alloc(rustc_middle::ty::layout::LayoutError::Unknown(ty)))
        }
    }

    pub fn remove_transparent_wrapper(&self, ty: Ty<'tcx>) -> Ty<'tcx> {
        if let TyKind::Adt(def, args) = ty.kind() {
            if def.repr().transparent() {
                let variant_0 = VariantIdx::from_u32(0);
                let v = &def.variants()[variant_0];
                let non_zst_field = v.fields.iter().find(|field| {
                    let field_ty = self.tcx.type_of(field.did).skip_binder();
                    let is_zst = self.layout_of(field_ty).is_ok_and(|layout| layout.is_zst());
                    !is_zst
                });
                if let Some(f) = non_zst_field {
                    return f.ty(self.tcx, args);
                }
            }
        }
        ty
    }

    #[logfn_inputs(TRACE)]
    fn specialize_const(
        &self,
        constant: Const<'tcx>,
        map: &Option<HashMap<rustc_span::Symbol, GenericArg<'tcx>>>,
    ) -> Const<'tcx> {
        if let ConstKind::Param(param_const) = constant.kind() {
            if let Some(gen_arg) = map.as_ref().unwrap().get(&param_const.name) {
                return gen_arg.expect_const();
            }
        }
        constant
    }

    #[logfn_inputs(TRACE)]
    fn specialize_generic_argument(
        &self,
        gen_arg: GenericArg<'tcx>,
        map: &Option<HashMap<rustc_span::Symbol, GenericArg<'tcx>>>,
    ) -> GenericArg<'tcx> {
        match gen_arg.unpack() {
            GenericArgKind::Type(ty) => self.specialize_type(ty, map).into(),
            GenericArgKind::Const(c) => self.specialize_const(c, map).into(),
            _ => gen_arg,
        }
    }

    #[logfn_inputs(TRACE)]
    pub fn specialize_type(
        &self,
        ty: Ty<'tcx>,
        map: &Option<HashMap<rustc_span::Symbol, GenericArg<'tcx>>>,
    ) -> Ty<'tcx> {
        // The projection of an associated type. For example,
        // `<T as Trait<..>>::N`.
        if let TyKind::Alias(rustc_middle::ty::Projection, projection) = ty.kind() {
            let specialized_substs = self.specialize_generic_args(projection.args, map);
            let item_def_id = projection.def_id;
            return if utils::are_concrete(specialized_substs) {
                let typing_env = self.get_typing_env_for(
                    self.tcx.associated_item(item_def_id).container_id(self.tcx),
                );
                if let Ok(Some(instance)) = rustc_middle::ty::Instance::try_resolve(
                    self.tcx,
                    typing_env,
                    item_def_id,
                    specialized_substs,
                ) {
                    let instance_item_def_id = instance.def.def_id();
                    if item_def_id == instance_item_def_id {
                        return Ty::new_projection(self.tcx, projection.def_id, specialized_substs);
                    }
                    let item_type = self.tcx.type_of(instance_item_def_id).skip_binder();
                    let map =
                        self.get_generic_arguments_map(instance_item_def_id, instance.args, &[]);
                    if item_type == ty && map.is_none() {
                        // Can happen if the projection just adds a life time
                        item_type
                    } else {
                        self.specialize_type(item_type, &map)
                    }
                } else {
                    let projection_trait = Some(self.tcx.parent(item_def_id));
                    if projection_trait == self.tcx.lang_items().pointee_trait() {
                        assume!(!specialized_substs.is_empty());
                        if let GenericArgKind::Type(ty) = specialized_substs[0].unpack() {
                            return ty.ptr_metadata_ty(self.tcx, |ty| ty);
                        }
                    } else if projection_trait == self.tcx.lang_items().discriminant_kind_trait() {
                        assume!(!specialized_substs.is_empty());
                        if let GenericArgKind::Type(enum_ty) = specialized_substs[0].unpack() {
                            return enum_ty.discriminant_ty(self.tcx);
                        }
                    }
                    debug!("could not resolve an associated type with concrete type arguments");
                    ty
                }
            } else {
                Ty::new_projection(self.tcx, projection.def_id, specialized_substs)
            };
        }
        if map.is_none() {
            return ty;
        }
        match ty.kind() {
            TyKind::Adt(def, args) => {
                Ty::new_adt(self.tcx, *def, self.specialize_generic_args(args, map))
            }
            TyKind::Array(elem_ty, len) => {
                let specialized_elem_ty = self.specialize_type(*elem_ty, map);
                let specialized_len = self.specialize_const(*len, map);
                self.tcx
                    .mk_ty_from_kind(TyKind::Array(specialized_elem_ty, specialized_len))
            }
            TyKind::Slice(elem_ty) => {
                let specialized_elem_ty = self.specialize_type(*elem_ty, map);
                Ty::new_slice(self.tcx, specialized_elem_ty)
            }
            TyKind::RawPtr(ty, mutbl) => {
                let specialized_ty = self.specialize_type(*ty, map);
                Ty::new_ptr(self.tcx, specialized_ty, *mutbl)
            }
            TyKind::Ref(region, ty, mutbl) => {
                let specialized_ty = self.specialize_type(*ty, map);
                Ty::new_ref(self.tcx, *region, specialized_ty, *mutbl)
            }
            TyKind::FnDef(def_id, args) => {
                Ty::new_fn_def(self.tcx, *def_id, self.specialize_generic_args(args, map))
            }
            TyKind::FnPtr(sig_tys, hdr) => {
                let fn_sig = sig_tys.with(*hdr);
                let map_fn_sig = |fn_sig: FnSig<'tcx>| {
                    let specialized_inputs_and_output = self.tcx.mk_type_list_from_iter(
                        fn_sig
                            .inputs_and_output
                            .iter()
                            .map(|ty| self.specialize_type(ty, map)),
                    );
                    FnSig {
                        inputs_and_output: specialized_inputs_and_output,
                        c_variadic: fn_sig.c_variadic,
                        safety: fn_sig.safety,
                        abi: fn_sig.abi,
                    }
                };
                let specialized_fn_sig = fn_sig.map_bound(map_fn_sig);
                Ty::new_fn_ptr(self.tcx, specialized_fn_sig)
            }
            TyKind::Dynamic(predicates, region, kind) => {
                let specialized_predicates = predicates.iter().map(
                    |bound_pred: rustc_middle::ty::Binder<'_, ExistentialPredicate<'tcx>>| {
                        bound_pred.map_bound(|pred| match pred {
                            ExistentialPredicate::Trait(ExistentialTraitRef {
                                def_id,
                                args,
                                ..
                            }) => ExistentialPredicate::Trait(ExistentialTraitRef::new_from_args(
                                self.tcx,
                                def_id,
                                self.specialize_generic_args(args, map),
                            )),
                            ExistentialPredicate::Projection(ExistentialProjection {
                                def_id,
                                args,
                                term,
                                ..
                            }) => {
                                if let Some(ty) = term.as_type() {
                                    ExistentialPredicate::Projection(
                                        ExistentialProjection::new_from_args(
                                            self.tcx,
                                            def_id,
                                            self.specialize_generic_args(args, map),
                                            self.specialize_type(ty, map).into(),
                                        ),
                                    )
                                } else {
                                    ExistentialPredicate::Projection(
                                        ExistentialProjection::new_from_args(
                                            self.tcx,
                                            def_id,
                                            self.specialize_generic_args(args, map),
                                            term,
                                        ),
                                    )
                                }
                            }
                            ExistentialPredicate::AutoTrait(_) => pred,
                        })
                    },
                );
                Ty::new_dynamic(
                    self.tcx,
                    self.tcx
                        .mk_poly_existential_predicates_from_iter(specialized_predicates),
                    *region,
                    *kind,
                )
            }
            TyKind::Closure(def_id, args) => {
                // Closure types can be part of their own type parameters...
                // so need to guard against endless recursion
                {
                    let mut borrowed_closures_being_specialized =
                        self.closures_being_specialized.borrow_mut();
                    let closures_being_specialized =
                        borrowed_closures_being_specialized.deref_mut();
                    if !closures_being_specialized.insert(*def_id) {
                        return ty;
                    }
                }
                let specialized_closure =
                    Ty::new_closure(self.tcx, *def_id, self.specialize_generic_args(args, map));
                let mut borrowed_closures_being_specialized =
                    self.closures_being_specialized.borrow_mut();
                let closures_being_specialized = borrowed_closures_being_specialized.deref_mut();
                closures_being_specialized.remove(def_id);
                specialized_closure
            }
            TyKind::Coroutine(def_id, args) => {
                Ty::new_coroutine(self.tcx, *def_id, self.specialize_generic_args(args, map))
            }
            TyKind::CoroutineWitness(def_id, args) => {
                let specialized_types = self.specialize_generic_args(args, map);
                Ty::new_coroutine_witness(self.tcx, *def_id, specialized_types)
            }
            TyKind::Tuple(types) => Ty::new_tup_from_iter(
                self.tcx,
                types.iter().map(|ty| self.specialize_type(ty, map)),
            ),
            TyKind::Alias(
                rustc_middle::ty::Opaque,
                rustc_middle::ty::AliasTy { def_id, args, .. },
            ) => Ty::new_opaque(self.tcx, *def_id, self.specialize_generic_args(args, map)),
            TyKind::Param(ParamTy { name, .. }) => {
                if let Some(map) = map {
                    if let Some(gen_arg) = map.get(name) {
                        return gen_arg.expect_ty();
                    }
                }
                ty
            }
            _ => ty,
        }
    }

    #[logfn_inputs(TRACE)]
    pub fn specialize_generic_args(
        &self,
        args: GenericArgsRef<'tcx>,
        map: &Option<HashMap<rustc_span::Symbol, GenericArg<'tcx>>>,
    ) -> GenericArgsRef<'tcx> {
        let specialized_generic_args: Vec<GenericArg<'_>> = args
            .iter()
            .map(|gen_arg| self.specialize_generic_argument(gen_arg, map))
            .collect();
        self.tcx.mk_args(&specialized_generic_args)
    }
}

pub fn is_transparent_wrapper(ty: Ty) -> bool {
    if let TyKind::Adt(def, _) = ty.kind() {
        def.repr().transparent()
    } else {
        false
    }
}

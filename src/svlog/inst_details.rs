// Copyright (c) 2016-2020 Fabian Schuiki

//! A collection if instantiation details.

#[warn(missing_docs)]
use crate::{
    crate_prelude::*,
    hir::{self, HirNode},
    Context, NodeEnvId, ParamEnv, ParamEnvData, ParamEnvSource, PortMapping, PortMappingSource,
};
use std::{ops::Deref, sync::Arc};

/// Instantiation details
///
/// This struct bundles all the information associated with an instantiation,
/// most importantly the parameter bindings and port connections.
///
/// This corresponds to the `bar(y)` in `foo #(x) bar(y);`.
#[derive(Debug, PartialEq, Eq)]
pub struct InstDetails<'a> {
    /// The HIR instantiation.
    pub inst: &'a hir::Inst<'a>,
    /// The target details.
    pub target: Arc<InstTargetDetails<'a>>,
    /// The port connections.
    pub ports: Arc<PortMapping>,
}

impl<'a> Deref for InstDetails<'a> {
    type Target = InstTargetDetails<'a>;

    fn deref(&self) -> &Self::Target {
        self.target.as_ref()
    }
}

/// Instantiation target details
///
/// This struct bundles all the information associated with an instantiation
/// target, most importantly the parameter bindings.
///
/// This corresponds to the `foo #(x)` in `foo #(x) bar(y);`.
#[derive(Debug, PartialEq, Eq)]
pub struct InstTargetDetails<'a> {
    /// The HIR instantiation target.
    pub inst_target: &'a hir::InstTarget,
    /// The instantiated HIR module.
    pub module: &'a hir::Module<'a>,
    /// The parameter environment around the instantiation.
    pub outer_env: ParamEnv,
    /// The parameter environment generated by the instantiation.
    pub inner_env: ParamEnv,
    /// The parameter bindings.
    pub params: &'a ParamEnvData<'a>,
}

pub(crate) fn compute_inst<'gcx>(
    cx: &impl Context<'gcx>,
    node: NodeEnvId,
) -> Result<Arc<InstDetails<'gcx>>> {
    // Look up the HIR of the instantiation.
    let inst = match cx.hir_of(node.id())? {
        HirNode::Inst(x) => x,
        x => bug_span!(cx.span(node.id()), cx, "inst_details called on a {:?}", x),
    };

    // Determine the details of the instantiation target.
    let target = cx.inst_target_details(inst.target.env(node.env()))?;

    // Determine the port connections of the instantiations. Connections
    // are made to the module's external ports, and must later be mapped
    // to the actual internal ports in a second step.
    let port_mapping = cx.port_mapping(PortMappingSource::ModuleInst {
        module: target.module.id,
        inst: node.id(),
        env: target.inner_env,
        pos: &inst.pos_ports,
        named: &inst.named_ports,
    })?;

    // Wrap everything up.
    Ok(Arc::new(InstDetails {
        inst,
        target: target,
        ports: port_mapping,
    }))
}

pub(crate) fn compute_inst_target<'gcx>(
    cx: &impl Context<'gcx>,
    node: NodeEnvId,
) -> Result<Arc<InstTargetDetails<'gcx>>> {
    // Look up the HIR of the instantiation target.
    let inst_target = match cx.hir_of(node.id())? {
        HirNode::InstTarget(x) => x,
        x => bug_span!(cx.span(node.id()), cx, "inst target is a {:?}", x),
    };

    // Resolve the name of the instantiated module.
    let module = match cx.gcx().find_module(inst_target.name.value) {
        Some(id) => id,
        None => {
            cx.emit(
                DiagBuilder2::error(format!(
                    "unknown module or interface `{}`",
                    inst_target.name.value
                ))
                .span(inst_target.name.span),
            );
            return Err(());
        }
    };

    // Look up the HIR of the instantiated module.
    let module_hir = match cx.hir_of(module)? {
        HirNode::Module(x) => x,
        x => bug_span!(cx.span(node.id()), cx, "instantiated module is a {:?}", x),
    };

    // Create a new parameter environment that is generated by the
    // parametrization of this instance.
    let inst_env = cx.param_env(ParamEnvSource::ModuleInst {
        module: module,
        inst: node.id(),
        env: node.env(),
        pos: &inst_target.pos_params,
        named: &inst_target.named_params,
    })?;
    let inst_env_data = cx.param_env_data(inst_env);

    // Wrap everything up.
    Ok(Arc::new(InstTargetDetails {
        inst_target,
        module: module_hir,
        outer_env: node.env(),
        inner_env: inst_env,
        params: inst_env_data,
    }))
}

/// A visitor that emits instantiation details diagnostics.
pub struct InstVerbosityVisitor<'a, 'gcx> {
    cx: &'a GlobalContext<'gcx>,
    env: ParamEnv,
}

impl<'a, 'gcx> InstVerbosityVisitor<'a, 'gcx> {
    /// Create a new visitor that emits instantiation details.
    pub fn new(cx: &'a GlobalContext<'gcx>) -> Self {
        Self {
            cx,
            env: cx.default_param_env(),
        }
    }
}

impl<'a, 'gcx> hir::Visitor<'gcx> for InstVerbosityVisitor<'a, 'gcx> {
    type Context = GlobalContext<'gcx>;

    fn context(&self) -> &Self::Context {
        self.cx
    }

    fn visit_inst(&mut self, hir: &'gcx hir::Inst) {
        let details = match self.cx.inst_details(hir.id.env(self.env)) {
            Ok(x) => x,
            Err(()) => return,
        };
        self.cx.emit(
            DiagBuilder2::note("instantiation details")
                .span(hir.name.span)
                .add_note(format!("{:#?}", details)),
        );
        Self {
            cx: self.cx,
            env: details.inner_env,
        }
        .visit_node_with_id(details.module.id, false);
    }
}

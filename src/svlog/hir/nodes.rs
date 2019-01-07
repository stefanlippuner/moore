// Copyright (c) 2017 Fabian Schuiki

//! This module contains the nodes of the tree structure that is the HIR.

use crate::crate_prelude::*;

/// A reference to an HIR node.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum HirNode<'hir> {
    Module(&'hir Module<'hir>),
    Port(&'hir Port),
    Type(&'hir Type),
    InstTarget(&'hir InstTarget),
    Inst(&'hir Inst<'hir>),
    TypeParam(&'hir TypeParam),
    // Interface(&'hir Interface),
    // Package(&'hir Package),
    // PortSlice(&'hir PortSlice),
    // ValueParam(&'hir ast::ParamValueDecl),
    // VarDecl(&'hir ast::VarDecl, &'hir ast::VarDeclName),
}

impl<'hir> HasSpan for HirNode<'hir> {
    fn span(&self) -> Span {
        match *self {
            HirNode::Module(x) => x.span(),
            HirNode::Port(x) => x.span(),
            HirNode::Type(x) => x.span(),
            HirNode::InstTarget(x) => x.span(),
            HirNode::Inst(x) => x.span(),
            HirNode::TypeParam(x) => x.span(),
        }
    }

    fn human_span(&self) -> Span {
        match *self {
            HirNode::Module(x) => x.human_span(),
            HirNode::Port(x) => x.human_span(),
            HirNode::Type(x) => x.human_span(),
            HirNode::InstTarget(x) => x.human_span(),
            HirNode::Inst(x) => x.human_span(),
            HirNode::TypeParam(x) => x.human_span(),
        }
    }
}

impl<'hir> HasDesc for HirNode<'hir> {
    fn desc(&self) -> &'static str {
        match *self {
            HirNode::Module(x) => x.desc(),
            HirNode::Port(x) => x.desc(),
            HirNode::Type(x) => x.desc(),
            HirNode::InstTarget(x) => x.desc(),
            HirNode::Inst(x) => x.desc(),
            HirNode::TypeParam(x) => x.desc(),
        }
    }

    fn desc_full(&self) -> String {
        match *self {
            HirNode::Module(x) => x.desc_full(),
            HirNode::Port(x) => x.desc_full(),
            HirNode::Type(x) => x.desc_full(),
            HirNode::InstTarget(x) => x.desc_full(),
            HirNode::Inst(x) => x.desc_full(),
            HirNode::TypeParam(x) => x.desc_full(),
        }
    }
}

/// A module.
#[derive(Debug, PartialEq, Eq)]
pub struct Module<'hir> {
    pub id: NodeId,
    pub name: Spanned<Name>,
    pub span: Span,
    // pub lifetime: ast::Lifetime,
    pub ports: &'hir [NodeId],
    pub params: &'hir [NodeId],
    // pub body: HierarchyBody,
    /// The module/interface instances in the module.
    pub insts: &'hir [NodeId],
}

impl HasSpan for Module<'_> {
    fn span(&self) -> Span {
        self.span
    }

    fn human_span(&self) -> Span {
        self.name.span
    }
}

impl HasDesc for Module<'_> {
    fn desc(&self) -> &'static str {
        "module"
    }

    fn desc_full(&self) -> String {
        format!("module `{}`", self.name.value)
    }
}

/// An instantiation target.
///
/// In an instantiation `foo #(...) a(), b(), c();` this struct represents the
/// `foo #(...)` part. Multiple instantiations (`a()`, `b()`, `c()`) may share
/// the same target.
#[derive(Debug, PartialEq, Eq)]
pub struct InstTarget {
    pub id: NodeId,
    pub name: Spanned<Name>,
    pub span: Span,
    pub pos_params: Vec<PosParam>,
    pub named_params: Vec<NamedParam>,
}

impl HasSpan for InstTarget {
    fn span(&self) -> Span {
        self.span
    }

    fn human_span(&self) -> Span {
        self.name.span
    }
}

impl HasDesc for InstTarget {
    fn desc(&self) -> &'static str {
        "instantiation"
    }

    fn desc_full(&self) -> String {
        format!("`{}` instantiation", self.name.value)
    }
}

/// A positional parameter.
pub type PosParam = (Span, NodeId);

/// A named parameter.
pub type NamedParam = (Span, Spanned<Name>, NodeId);

/// An instantiation.
///
/// In an instantiation `foo #(...) a(), b(), c();`, this struct represents the
/// `a()` part.
#[derive(Debug, PartialEq, Eq)]
pub struct Inst<'hir> {
    pub id: NodeId,
    pub name: Spanned<Name>,
    pub span: Span,
    /// The target of the instantiation.
    pub target: NodeId,
    pub dummy: std::marker::PhantomData<&'hir ()>,
}

impl HasSpan for Inst<'_> {
    fn span(&self) -> Span {
        self.span
    }

    fn human_span(&self) -> Span {
        self.name.span
    }
}

impl HasDesc for Inst<'_> {
    fn desc(&self) -> &'static str {
        "instance"
    }

    fn desc_full(&self) -> String {
        format!("instance `{}`", self.name.value)
    }
}

/// A type parameter.
#[derive(Debug, PartialEq, Eq)]
pub struct TypeParam {
    pub id: NodeId,
    pub name: Spanned<Name>,
    pub span: Span,
    pub local: bool,
    pub default: Option<NodeId>,
}

impl HasSpan for TypeParam {
    fn span(&self) -> Span {
        self.span
    }

    fn human_span(&self) -> Span {
        self.name.span
    }
}

impl HasDesc for TypeParam {
    fn desc(&self) -> &'static str {
        "type parameter"
    }

    fn desc_full(&self) -> String {
        format!("type parameter `{}`", self.name.value)
    }
}

/// An interface.
pub struct Interface {
    pub id: NodeId,
    pub name: Name,
    pub span: Span,
    pub lifetime: ast::Lifetime,
    pub ports: Vec<Port>,
    pub params: Vec<ast::ParamDecl>,
    pub body: HierarchyBody,
}

/// A package.
pub struct Package {
    pub name: Name,
    pub span: Span,
    pub lifetime: ast::Lifetime,
    pub body: HierarchyBody,
}

/// A hierarchy body represents the contents of a module, interface, or package.
/// Generate regions and nested modules introduce additional bodies. The point
/// of hierarchy bodies is to take a level of the design hierarchy and group all
/// declarations by type, rather than having them in a single array in
/// declaration order.
pub struct HierarchyBody {
    pub procs: Vec<ast::Procedure>,
    pub nets: Vec<ast::NetDecl>,
    pub vars: Vec<ast::VarDecl>,
    pub assigns: Vec<ast::ContAssign>,
    pub params: Vec<ast::ParamDecl>,
    pub insts: Vec<ast::Inst>,
    pub genreg: Vec<HierarchyBody>,
    pub genvars: Vec<ast::GenvarDecl>,
    pub genfors: Vec<GenerateFor>,
    pub genifs: Vec<GenerateIf>,
    pub gencases: Vec<ast::GenerateCase>,
    pub classes: Vec<ast::ClassDecl>, // TODO: Make this an HIR node, since it contains hierarchy items
    pub subroutines: Vec<ast::SubroutineDecl>, // TODO: Make this an HIR node
    pub asserts: Vec<ast::Assertion>,
    pub typedefs: Vec<ast::Typedef>,
}

/// A module or interface port.
#[derive(Debug, PartialEq, Eq)]
pub struct Port {
    pub id: NodeId,
    pub name: Spanned<Name>,
    pub span: Span,
    pub dir: ast::PortDir,
    pub ty: NodeId,
    // pub slices: Vec<PortSlice>,
}

impl HasSpan for Port {
    fn span(&self) -> Span {
        self.span
    }

    fn human_span(&self) -> Span {
        self.name.span
    }
}

impl HasDesc for Port {
    fn desc(&self) -> &'static str {
        "port"
    }

    fn desc_full(&self) -> String {
        format!("port `{}`", self.name.value)
    }
}

/// A port slice refers to a port declaration within the module. It consists of
/// the name of the declaration and a list of optional member and index accesses
/// that select individual parts of the declaration.
#[derive(Debug)]
pub struct PortSlice {
    pub id: NodeId,
    pub name: Name,
    pub span: Span,
    pub selects: Vec<PortSelect>,
    pub dir: ast::PortDir,
    pub kind: ast::PortKind,
    pub ty: Option<ast::Type>,
    pub dims: Vec<ast::TypeDim>,
}

#[derive(Debug)]
pub enum PortSelect {
    Member(Span, Name),
    Index(Span, Expr),
}

pub struct PortDecl {
    pub dir: ast::PortDir,
}

// pub enum PortDir {

// }

#[derive(Debug)]
pub struct Expr;

pub struct GenerateBlock {
    pub span: Span,
    pub label: Option<Name>,
    pub body: HierarchyBody,
}

pub struct GenerateFor {
    pub span: Span,
    pub init: ast::Stmt,
    pub cond: ast::Expr,
    pub step: ast::Expr,
    pub block: GenerateBlock,
}

pub struct GenerateIf {
    pub span: Span,
    pub cond: ast::Expr,
    pub main_block: GenerateBlock,
    pub else_block: Option<GenerateBlock>,
}

/// A type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Type {
    pub id: NodeId,
    pub span: Span,
    pub kind: TypeKind,
}

impl HasSpan for Type {
    fn span(&self) -> Span {
        self.span
    }
}

impl HasDesc for Type {
    fn desc(&self) -> &'static str {
        #[allow(unreachable_patterns)]
        match self.kind {
            TypeKind::Builtin(BuiltinType::Void) => "void type",
            TypeKind::Builtin(BuiltinType::Bit) => "bit type",
            TypeKind::Builtin(BuiltinType::Logic) => "logic type",
            TypeKind::Builtin(BuiltinType::Byte) => "byte type",
            TypeKind::Builtin(BuiltinType::ShortInt) => "short int type",
            TypeKind::Builtin(BuiltinType::Int) => "int type",
            TypeKind::Builtin(BuiltinType::LongInt) => "long int type",
            _ => "type",
        }
    }

    fn desc_full(&self) -> String {
        match self.kind {
            TypeKind::Named(n) => format!("type `{}`", n.value),
            _ => self.desc().into(),
        }
    }
}

/// The different forms a type can take.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeKind {
    /// A builtin type.
    Builtin(BuiltinType),
    /// A named type.
    Named(Spanned<Name>),
}

/// A builtin type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinType {
    Void,
    Bit,
    Logic,
    Byte,
    ShortInt,
    Int,
    LongInt,
}

use std::collections::HashMap;

use koopa::ir::*;
use koopa::ir::builder_traits::*;

/// 作用域中的条目：要么是编译期常量，要么是运行时变量（存 alloc handle）。
#[derive(Debug)]
pub enum ScopeEntry {
    Const(i32),
    Var(Value),
}

/// 支持作用域嵌套的符号表。
///
/// - `scopes`：名字 → 条目，栈顶为当前作用域。管"名字可见性"。
/// - `value_cache`：`Value` handle → 已知的编译期值。管"值缓存"。
///   与作用域无关——`Value` handle 全局唯一，无论变量还是中间表达式结果都能缓存。
#[derive(Debug)]
pub struct SymTable {
    scopes: Vec<HashMap<String, ScopeEntry>>,
    pub value_cache: HashMap<Value, i32>,
}

impl SymTable {
    pub fn new() -> Self {
        SymTable {
            scopes: vec![HashMap::new()],
            value_cache: HashMap::new(),
        }
    }

    /// 进入代码块时调用，新建一层作用域。
    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    /// 退出代码块时调用，销毁最内层作用域。
    pub fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    /// 在当前作用域插入符号。若当前作用域已存在同名符号则 panic。
    pub fn insert(&mut self, key: String, entry: ScopeEntry) {
        let cur = self.scopes.last_mut().unwrap();
        if cur.contains_key(&key) {
            panic!("redefined symbol: {}", key);
        }
        cur.insert(key, entry);
    }

    /// 跨作用域查找符号。从当前作用域向外逐层查找，找不到返回 `None`。
    pub fn get(&self, key: &str) -> Option<&ScopeEntry> {
        for scope in self.scopes.iter().rev() {
            if let Some(v) = scope.get(key) {
                return Some(v);
            }
        }
        None
    }
}

/// 为二元表达式枚举统一生成 convert_to_koopa_ir 和 evaluate 方法。
///
/// 用法：
/// ```ignore
/// impl_binary_expr!(AddExp, leaf: Mul(MulExp),
///     variants: [
///         Add => BinaryOp::Add, eval: |l, r| l + r,
///         Sub => BinaryOp::Sub, eval: |l, r| l - r,
///     ]
/// );
/// ```
/// `eval:` 后跟一个闭包 `|l, r| expr`，`l` / `r` 为已求值的 `i32`。
macro_rules! impl_binary_expr {
    (
        $enum:ident, leaf: $leaf_variant:ident($leaf_ty:ty),
        variants: [$(
            $variant:ident => $binary_op:expr, eval: $eval_body:expr
        ),* $(,)?]
    ) => {
        impl $enum {
            fn convert_to_koopa_ir(&self, func_data: &mut FunctionData, entry: BasicBlock,
                symtable: &mut SymTable) -> Value {
                if let Some(val) = self.evaluate(symtable) {
                    return func_data.dfg_mut().new_value().integer(val);
                }
                match self {
                    Self::$leaf_variant(inner) => inner.convert_to_koopa_ir(func_data, entry, symtable),
                    $(
                        Self::$variant(lhs, rhs) => {
                            let lv = lhs.convert_to_koopa_ir(func_data, entry, symtable);
                            let rv = rhs.convert_to_koopa_ir(func_data, entry, symtable);
                            let v = func_data.dfg_mut().new_value().binary($binary_op, lv, rv);
                            let _ = func_data.layout_mut().bb_mut(entry).insts_mut().push_key_back(v);
                            v
                        }
                    )*
                }
            }

            fn evaluate(&self, symtable: &SymTable) -> Option<i32> {
                match self {
                    Self::$leaf_variant(inner) => inner.evaluate(symtable),
                    $(
                        Self::$variant(lhs, rhs) => {
                            let l = lhs.evaluate(symtable)?;
                            let r = rhs.evaluate(symtable)?;
                            Some(($eval_body)(l, r))
                        }
                    )*
                }
            }
        }
    };
}

#[derive(Debug)]
pub struct CompUnit {
  pub func_def: FuncDef,
}
impl CompUnit {
    pub fn convert_to_koopa_ir(&mut self) -> Program {
        let mut program = Program::new();
        self.func_def.convert_to_koopa_ir(&mut program);
        // program.new_func_def(func);
        program
    }
}

#[derive(Debug)]
pub struct FuncDef {
  pub func_type: FuncType,
  pub ident: String,
  pub block: Block,
  pub symtable: SymTable,
}
impl FuncDef {
    fn convert_to_koopa_ir(&mut self, program: &mut Program){
        let name = format!("@{}", self.ident);
        let func = program.new_func_def(name, vec![], Type::get_i32());
        let func_data = program.func_mut(func);
        let entry = func_data.dfg_mut().new_bb().basic_block(Some("%entry".into()));
        let _ = func_data.layout_mut().bbs_mut().push_key_back(entry);
        self.block.convert_to_koopa_ir(func_data, entry, &mut self.symtable);
    }
}

#[derive(Debug)]
pub enum FuncType {
  Void,
  Int,
}

#[derive(Debug)]
pub struct Block {
  pub items: Vec<BlockItem>,
}
impl Block {
    fn convert_to_koopa_ir(&self, func_data: &mut FunctionData,
        entry:BasicBlock, symtable: &mut SymTable){
        symtable.push_scope();
        for item in &self.items {
            match item {
                BlockItem::Decl(decl) => decl.convert_to_koopa_ir(func_data, entry, symtable),
                BlockItem::Stmt(stmt) => stmt.convert_to_koopa_ir(func_data, entry, symtable),
            }
        }
        symtable.pop_scope();
    }
}

#[derive(Debug)]
pub enum BlockItem {
    Decl(Decl),
    Stmt(Stmt),
}

#[derive(Debug)]
pub enum Decl {
    ConstDecl(ConstDecl),
    VarDecl(VarDecl)
}
impl Decl {
    fn convert_to_koopa_ir(&self, func_data: &mut FunctionData, entry: koopa::ir::BasicBlock,
         symtable: &mut SymTable) {
        match self {
            Decl::ConstDecl(cd) => cd.convert_to_koopa_ir(symtable),
            Decl::VarDecl(vd)=> vd.convert_to_koopa_ir(func_data, entry, symtable),
        }
    }
}

#[derive(Debug)]
pub enum BType {
    Int,
}

#[derive(Debug)]
pub struct ConstDecl {
    pub btype: BType,
    pub defs: Vec<ConstDef>,
}
impl ConstDecl {
    fn convert_to_koopa_ir(&self, symtable: &mut SymTable) {
        for def in &self.defs {
            let val = def.init_val.evaluate(symtable).unwrap();
            symtable.insert(def.ident.clone(), ScopeEntry::Const(val));
        }
    }
}

#[derive(Debug)]
pub struct ConstDef {
    pub ident: String,
    pub init_val: ConstInitVal,
}

#[derive(Debug)]
pub enum ConstInitVal {
    Exp(ConstExp),
}
impl ConstInitVal {
    fn evaluate(&self, symtable: &SymTable) -> Option<i32> {
        match self {
            ConstInitVal::Exp(exp) => exp.evaluate(symtable),
        }
    }
}

#[derive(Debug)]
pub struct ConstExp {
    pub exp: Exp,
}
impl ConstExp {
    fn evaluate(&self, symtable: &SymTable) -> Option<i32> {
        self.exp.evaluate(symtable)
    }
}
#[derive(Debug)]
pub struct VarDecl {
    pub btype: BType,
    pub defs: Vec<VarDef>,
}
impl VarDecl {
    fn convert_to_koopa_ir(&self, func_data: &mut FunctionData, entry: koopa::ir::BasicBlock, 
        symtable: &mut SymTable) {
        for def in &self.defs {
            let ty = match self.btype{
                BType::Int=>Type::get_i32()
            };
            let alloc = func_data.dfg_mut().new_value().alloc(ty);
            let _ = func_data.layout_mut().bb_mut(entry).insts_mut().push_key_back(alloc);
            
            match def{
                VarDef::IDENT(name)=>{
                    symtable.insert(name.clone(), ScopeEntry::Var(alloc));
                },
                VarDef::IDENTInitVal(name,initval )=>{
                    let val = initval.evaluate(symtable).unwrap();
                    let value = func_data.dfg_mut().new_value().integer(val);
                    let store = func_data.dfg_mut().new_value().store(value, alloc);
                    let _ = func_data.layout_mut().bb_mut(entry).
                        insts_mut().push_key_back(store);
                    symtable.insert(name.clone(), ScopeEntry::Var(alloc));
                    symtable.value_cache.insert(alloc, val);
                }
            }
        }
    }
}

#[derive(Debug)]
pub enum VarDef {
    IDENT(String),
    IDENTInitVal(String,InitVal),
}

#[derive(Debug)]
pub struct InitVal {
    pub exp: Exp,
}
impl InitVal {
    fn evaluate(&self, symtable: &SymTable) -> Option<i32> {
        self.exp.evaluate(symtable)
    }
}

#[derive(Debug)]
pub enum Stmt {
  Return(Exp),
  Assign(LVal, Exp),
  ExprStmt(Option<Exp>),
  Block(Block),
}
impl Stmt {
    fn convert_to_koopa_ir(&self, func_data: &mut FunctionData, entry: koopa::ir::BasicBlock,
        symtable: &mut SymTable){
        match self{
            Stmt::Assign(lval,exp )=>{
                let alloc = match symtable.get(&lval.ident).unwrap() {
                    ScopeEntry::Const(_) => panic!("should not assign to const"),
                    ScopeEntry::Var(v) => *v,
                };
                let expval = exp.convert_to_koopa_ir(func_data, entry, symtable);
                let store = func_data.dfg_mut().new_value().store(expval, alloc);
                let _ = func_data.layout_mut().bb_mut(entry).
                    insts_mut().push_key_back(store);
                // 尝试编译期求值并更新缓存
                if let Some(newval) = exp.evaluate(symtable) {
                    symtable.value_cache.insert(alloc, newval);
                }
            },
            Stmt::Return(exp)=>{
                let value = exp.convert_to_koopa_ir(func_data, entry, symtable);
                let ret = func_data.dfg_mut().new_value().ret(Some(value));
                let _ = func_data.layout_mut().bb_mut(entry).insts_mut().push_key_back(ret);
            },
            Stmt::ExprStmt(exp_opt)=>{
                // [Exp] ";" — 只有表达式的语句
                if let Some(exp) = exp_opt {
                    exp.convert_to_koopa_ir(func_data, entry, symtable);
                }
            },
            Stmt::Block(block)=>{
                block.convert_to_koopa_ir(func_data, entry, symtable);
            }
        }
    }
}

#[derive(Debug)]
pub struct Exp{
    pub lorexp: LOrExp,
}
impl Exp {
    fn convert_to_koopa_ir(&self, func_data: &mut FunctionData, entry: BasicBlock,
        symtable: &mut SymTable) -> Value {
        self.lorexp.convert_to_koopa_ir(func_data, entry, symtable)
    }
    fn evaluate(&self, symtable: &SymTable) -> Option<i32> {
        self.lorexp.evaluate(symtable)
    }
}


#[derive(Debug)]
pub enum UnaryExp{
    Primary(PrimaryExp),
    Unary(UnaryOp, Box<UnaryExp>),
}
impl UnaryExp {
    fn convert_to_koopa_ir(&self, func_data: &mut FunctionData, entry: BasicBlock,
        symtable: &mut SymTable) -> Value {
        if let Some(val) = self.evaluate(symtable) {
            return func_data.dfg_mut().new_value().integer(val);
        }
        match self {
            UnaryExp::Primary(primary) => primary.convert_to_koopa_ir(func_data, entry, symtable),
            UnaryExp::Unary(op, unary) => {
                let val = unary.convert_to_koopa_ir(func_data, entry, symtable);
                match op {
                    UnaryOp::Pos => val,  // 恒等，handle 不变，不需要额外缓存
                    UnaryOp::Neg => {
                        let zero = func_data.dfg_mut().new_value().integer(0);
                        let sub = func_data.dfg_mut().new_value().binary(BinaryOp::Sub, zero, val);
                        let _ = func_data.layout_mut().bb_mut(entry).insts_mut().push_key_back(sub);
                        sub
                    },
                    UnaryOp::Not => {
                        let zero = func_data.dfg_mut().new_value().integer(0);
                        let eq = func_data.dfg_mut().new_value().binary(BinaryOp::Eq, val, zero);
                        let _ = func_data.layout_mut().bb_mut(entry).insts_mut().push_key_back(eq);
                        eq
                    },
                }
            }
        }
    }
    fn evaluate(&self, symtable: &SymTable) -> Option<i32> {
        match self {
            UnaryExp::Primary(primary) => primary.evaluate(symtable),
            UnaryExp::Unary(op, unary) => {
                let val = unary.evaluate(symtable)?;
                match op {
                    UnaryOp::Pos => Some(val),
                    UnaryOp::Neg => Some(-val),
                    UnaryOp::Not => Some(if val == 0 { 1 } else { 0 }),
                }
            }
        }
    }
}

#[derive(Debug)]
pub enum UnaryOp{
    Pos,
    Neg,
    Not
}

#[derive(Debug)]
pub struct LVal {
    pub ident: String,
}

#[derive(Debug)]
pub enum PrimaryExp{
    Exp(Box<Exp>),
    LVal(LVal),
    Number(i32),
}
impl PrimaryExp {
    fn convert_to_koopa_ir(&self, func_data: &mut FunctionData, entry: BasicBlock,
        symtable: &mut SymTable) -> Value {
        match self {
            PrimaryExp::Exp(exp) => exp.convert_to_koopa_ir(func_data, entry, symtable),
            PrimaryExp::LVal(lval) => {
                let entry_s = symtable.get(&lval.ident)
                    .expect(&format!("undefined variable: {}", lval.ident));
                match entry_s {
                    ScopeEntry::Const(c) => {
                        let v = func_data.dfg_mut().new_value().integer(*c);
                        symtable.value_cache.insert(v, *c);
                        v
                    },
                    ScopeEntry::Var(alloc) => {
                        // 有缓存值就用整数常量，否则生成 load
                        if let Some(cached) = symtable.value_cache.get(alloc) {
                            let v = func_data.dfg_mut().new_value().integer(*cached);
                            symtable.value_cache.insert(v, *cached);
                            v
                        } else {
                            let load = func_data.dfg_mut().new_value().load(*alloc);
                            let _ = func_data.layout_mut().bb_mut(entry).insts_mut().push_key_back(load);
                            load
                        }
                    }
                }
            },
            PrimaryExp::Number(n) => {
                let v = func_data.dfg_mut().new_value().integer(*n);
                symtable.value_cache.insert(v, *n);
                v
            },
        }
    }
    fn evaluate(&self, symtable: &SymTable) -> Option<i32> {
        match self {
            PrimaryExp::Exp(exp) => exp.evaluate(symtable),
            PrimaryExp::LVal(lval) => {
                let entry_s = symtable.get(&lval.ident)
                    .expect(&format!("undefined variable: {}", lval.ident));
                match entry_s {
                    ScopeEntry::Const(c) => Some(*c),
                    ScopeEntry::Var(alloc) => symtable.value_cache.get(alloc).copied(),
                }
            },
            PrimaryExp::Number(n) => Some(*n),
        }
    }
}

#[derive(Debug)]
pub enum AddExp{
    Mul(MulExp),
    Add(Box<AddExp>, MulExp),
    Sub(Box<AddExp>, MulExp),
}
impl_binary_expr!(AddExp, leaf: Mul(MulExp),
    variants: [
        Add => BinaryOp::Add, eval: |l, r| l + r,
        Sub => BinaryOp::Sub, eval: |l, r| l - r,
    ]
);

#[derive(Debug)]
pub enum MulExp{
    Unary(UnaryExp),
    Mul(Box<MulExp>, UnaryExp),
    Div(Box<MulExp>, UnaryExp),
    Mod(Box<MulExp>, UnaryExp),
}
impl_binary_expr!(MulExp, leaf: Unary(UnaryExp),
    variants: [
        Mul => BinaryOp::Mul, eval: |l, r| l * r,
        Div => BinaryOp::Div, eval: |l, r| l / r,
        Mod => BinaryOp::Mod, eval: |l, r| l % r,
    ]
);

#[derive(Debug)]
pub enum RelExp{
    Add(AddExp),
    Lt(Box<RelExp>, AddExp),
    Gt(Box<RelExp>, AddExp),
    Le(Box<RelExp>, AddExp),
    Ge(Box<RelExp>, AddExp),
}
impl_binary_expr!(RelExp, leaf: Add(AddExp),
    variants: [
        Lt => BinaryOp::Lt, eval: |l, r| (l < r) as i32,
        Gt => BinaryOp::Gt, eval: |l, r| (l > r) as i32,
        Le => BinaryOp::Le, eval: |l, r| (l <= r) as i32,
        Ge => BinaryOp::Ge, eval: |l, r| (l >= r) as i32,
    ]
);

#[derive(Debug)]
pub enum EqExp{
    Rel(RelExp),
    Eq(Box<EqExp>, RelExp),
    Ne(Box<EqExp>, RelExp),
}
impl_binary_expr!(EqExp, leaf: Rel(RelExp),
    variants: [
        Eq => BinaryOp::Eq, eval: |l, r| (l == r) as i32,
        Ne => BinaryOp::NotEq, eval: |l, r| (l != r) as i32,
    ]
);

#[derive(Debug)]
pub enum LAndExp{
    Eq(EqExp),
    And(Box<LAndExp>, EqExp),
}
impl_binary_expr!(LAndExp, leaf: Eq(EqExp),
    variants: [
        And => BinaryOp::And,
            eval: |l, r| if l != 0 && r != 0 { 1 } else { 0 },
    ]
);

#[derive(Debug)]
pub enum LOrExp{
    And(LAndExp),
    Or(Box<LOrExp>, LAndExp),
}
impl_binary_expr!(LOrExp, leaf: And(LAndExp),
    variants: [
        Or => BinaryOp::Or,
            eval: |l, r| if l != 0 || r != 0 { 1 } else { 0 },
    ]
);

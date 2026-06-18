use std::collections::HashMap;

use koopa::ir::*;
use koopa::ir::builder_traits::*;

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
                symtable: &mut HashMap<String, ValInfo>) -> Value {
                if let Ok(Some(val)) = self.evaluate(symtable) {
                    return func_data.dfg_mut().new_value().integer(val);
                }
                match self {
                    Self::$leaf_variant(inner) => inner.convert_to_koopa_ir(func_data, entry, symtable),
                    $(
                        Self::$variant(lhs, rhs) => {
                            let lhs = lhs.convert_to_koopa_ir(func_data, entry, symtable);
                            let rhs = rhs.convert_to_koopa_ir(func_data, entry, symtable);
                            let v = func_data.dfg_mut().new_value().binary($binary_op, lhs, rhs);
                            let _ = func_data.layout_mut().bb_mut(entry).insts_mut().push_key_back(v);
                            v
                        }
                    )*
                }
            }

            fn evaluate(&self, symtable: &HashMap<String, ValInfo>)
                -> Result<Option<i32>, &'static str> {
                match self {
                    Self::$leaf_variant(inner) => inner.evaluate(symtable),
                    $(
                        Self::$variant(lhs, rhs) => {
                            Ok(Some(($eval_body)(
                                lhs.evaluate(symtable)?.unwrap(),
                                rhs.evaluate(symtable)?.unwrap(),
                            )))
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
  pub symtable: HashMap<String, ValInfo>,
}
#[derive(Debug)]
pub enum ValInfo{
    Const(i32),
    Var(Value, Option<i32>)
}
impl FuncDef {
    fn convert_to_koopa_ir(&mut self, program: &mut Program){
        let name = format!("@{}", self.ident);
        let func = program.new_func_def(name, vec![], Type::get_i32());
        let func_data = program.func_mut(func);
        self.block.convert_to_koopa_ir(func_data, &mut self.symtable);
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
        symtable: &mut HashMap<String, ValInfo>){
        let entry = func_data.dfg_mut().new_bb().basic_block(Some("%entry".into()));
        let _ = func_data.layout_mut().bbs_mut().push_key_back(entry);
        for item in &self.items {
            match item {
                BlockItem::Decl(decl) => decl.convert_to_koopa_ir(func_data, entry, symtable),
                BlockItem::Stmt(stmt) => stmt.convert_to_koopa_ir(func_data, entry, symtable),
            }
        }
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
         symtable: &mut HashMap<String, ValInfo>) {
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
    fn convert_to_koopa_ir(&self, symtable: &mut HashMap<String, ValInfo>) {
        for def in &self.defs {
            let val = def.init_val.evaluate(symtable);
            symtable.insert(def.ident.clone(), ValInfo::Const(val.unwrap().unwrap()));
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
    fn evaluate(&self, symtable: &HashMap<String, ValInfo>) 
    -> Result<Option<i32>, &'static str> {
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
    fn evaluate(&self, symtable: &HashMap<String, ValInfo>) 
    -> Result<Option<i32>, &'static str> {
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
        symtable: &mut HashMap<String, ValInfo>) {
        for def in &self.defs {
            let ty = match self.btype{
                BType::Int=>Type::get_i32()
            };
            let alloc = func_data.dfg_mut().new_value().alloc(ty);
            let _ = func_data.layout_mut().bb_mut(entry).insts_mut().push_key_back(alloc);
            
            match def{
                VarDef::IDENT(name)=>{
                    if symtable.get(name).is_none(){
                        symtable.insert(name.clone(), ValInfo::Var(alloc,None));
                    }else{
                        panic!("repeat declare");
                    }
                },
                VarDef::IDENTInitVal(name,initval )=>{
                    let val = initval.evaluate(symtable).unwrap().unwrap();
                    let value = func_data.dfg_mut().new_value().integer(val);
                    let store = func_data.dfg_mut().new_value().store(value, alloc);
                    let _ = func_data.layout_mut().bb_mut(entry).
                        insts_mut().push_key_back(store);
                    if symtable.get(name).is_none(){
                        symtable.insert(name.clone(), ValInfo::Var(alloc,Some(val)));
                    }else{
                        panic!("repeat declare");
                    }
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
    fn evaluate(&self, symtable: &HashMap<String, ValInfo>) -> Result<Option<i32>, &'static str> {
        self.exp.evaluate(symtable)
    }
}

#[derive(Debug)]
pub enum Stmt {
  Return(Exp),
  Assign(LVal, Exp)
}
impl Stmt {
    fn convert_to_koopa_ir(&self, func_data: &mut FunctionData, entry: koopa::ir::BasicBlock,
        symtable: &mut HashMap<String, ValInfo>){
        match self{
            Stmt::Assign(lval,exp )=>{
                let v = symtable.get(&lval.ident).unwrap();
                match v{
                    ValInfo::Const(_)=>{
                        panic!("should not assign to const")
                    },
                    ValInfo::Var(v,_)=>{        
                        let v = *v;  
                        let expval = exp.convert_to_koopa_ir(func_data, entry, symtable);              
                        let store = func_data.dfg_mut().new_value().store(expval, v);
                        let _ = func_data.layout_mut().bb_mut(entry).
                        insts_mut().push_key_back(store);
                        let newval = exp.evaluate(symtable).unwrap().unwrap();
                        // update the value for var
                        symtable.insert(lval.ident.clone(), ValInfo::Var(v,Some(newval)));
                    }
                }
            },
            Stmt::Return(exp)=>{
                let value = exp.convert_to_koopa_ir(func_data, entry, symtable);
                let ret = func_data.dfg_mut().new_value().ret(Some(value));
                let _ = func_data.layout_mut().bb_mut(entry).insts_mut().push_key_back(ret);
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
        symtable: &mut HashMap<String, ValInfo>) -> Value {
        self.lorexp.convert_to_koopa_ir(func_data, entry, symtable)
    }
    fn evaluate(&self, symtable: &HashMap<String, ValInfo>) -> Result<Option<i32>, &'static str> {
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
        symtable: &mut HashMap<String, ValInfo>) -> Value {
        if let Ok(Some(val)) = self.evaluate(symtable) {
            return func_data.dfg_mut().new_value().integer(val);
        }
        match self {
            UnaryExp::Primary(primary) => primary.convert_to_koopa_ir(func_data, entry, symtable),
            UnaryExp::Unary(op, unary) => {
                let val = unary.convert_to_koopa_ir(func_data, entry, symtable);
                match op {
                    UnaryOp::Pos => val,
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
    fn evaluate(&self, symtable: &HashMap<String, ValInfo>) -> Result<Option<i32>, &'static str> {
        match self {
            UnaryExp::Primary(primary) => primary.evaluate(symtable),
            UnaryExp::Unary(op, unary) => {
                let val = unary.evaluate(symtable)?;
                match op {
                    UnaryOp::Pos => Ok(val),
                    UnaryOp::Neg => Ok(Some(-val.unwrap())),
                    UnaryOp::Not => Ok(if val == Some(0) { Some(1) } else { Some(0) }),
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
        symtable: &mut HashMap<String, ValInfo>) -> Value {
        match self {
            PrimaryExp::Exp(exp) => exp.convert_to_koopa_ir(func_data, entry, symtable),
            PrimaryExp::LVal(lval) => {
                // 查符号表获取常量值
                let val = symtable.get(&lval.ident)
                    .expect(&format!("undefined variable: {}", lval.ident));
                match val{
                    ValInfo::Const(c)=>{
                        func_data.dfg_mut().new_value().integer(*c)
                    },
                    ValInfo::Var(v,val_opt)=>{
                        // let integer = func_data.dfg_mut().new_value().integer(val_opt.unwrap());
                        // let _ = func_data.layout_mut().bb_mut(entry).insts_mut().push_key_back(integer);
                        // integer
                        let load = func_data.dfg_mut().new_value().load(*v);
                        let _ = func_data.layout_mut().bb_mut(entry).insts_mut().push_key_back(load);
                        load
                    }
                }
            },
            PrimaryExp::Number(n) => func_data.dfg_mut().new_value().integer(*n),
        }
    }
    fn evaluate(&self, symtable: &HashMap<String, ValInfo>) -> Result<Option<i32>, &'static str> {
        match self {
            PrimaryExp::Exp(exp) => exp.evaluate(symtable),
            PrimaryExp::LVal(lval) => {
                let val =  symtable.get(&lval.ident)
                .expect(&format!("undefined variable: {}", lval.ident));
                match val{
                    ValInfo::Const(c)=>{
                        Ok(Some(*c))
                    },
                    ValInfo::Var(v,i32_opt)=>{
                        Ok(*i32_opt)
                    }
                }
            },
            PrimaryExp::Number(n) => Ok(Some(*n)),
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

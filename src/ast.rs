use std::collections::HashMap;

use koopa::ir::*;
use koopa::ir::builder_traits::*;

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
  pub symtable: HashMap<String, i32>,
}
impl FuncDef {
    fn convert_to_koopa_ir(&mut self, program: &mut Program){
        match self.func_type {
            FuncType::Void => {
                let name = format!("@{}", self.ident);
                let func = program.new_func_def(name, vec![], Type::get_i32());
                let func_data = program.func_mut(func);
                self.block.convert_to_koopa_ir(func_data, &mut self.symtable);
            },
            FuncType::Int => {
                let name = format!("@{}", self.ident);
                let func = program.new_func_def(name, vec![], Type::get_i32());
                let func_data = program.func_mut(func);
                self.block.convert_to_koopa_ir(func_data, &mut self.symtable);
            },
        }
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
    fn convert_to_koopa_ir(&self, func_data: &mut FunctionData, symtable: &mut HashMap<String, i32>){
        let entry = func_data.dfg_mut().new_bb().basic_block(Some("%entry".into()));
        let _ = func_data.layout_mut().bbs_mut().push_key_back(entry);
        for item in &self.items {
            match item {
                BlockItem::Decl(decl) => decl.convert_to_koopa_ir(symtable),
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
}
impl Decl {
    fn convert_to_koopa_ir(&self, symtable: &mut HashMap<String, i32>) {
        match self {
            Decl::ConstDecl(cd) => cd.convert_to_koopa_ir(symtable),
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
    fn convert_to_koopa_ir(&self, symtable: &mut HashMap<String, i32>) {
        for def in &self.defs {
            let val = def.init_val.evaluate(symtable);
            symtable.insert(def.ident.clone(), val.unwrap());
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
    fn evaluate(&self, symtable: &HashMap<String, i32>) -> Result<i32, &'static str> {
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
    fn evaluate(&self, symtable: &HashMap<String, i32>) -> Result<i32, &'static str> {
        self.exp.evaluate(symtable)
    }
}

#[derive(Debug)]
pub struct Stmt {
  pub exp: Exp,
}
impl Stmt {
    fn convert_to_koopa_ir(&self, func_data: &mut FunctionData, entry: koopa::ir::BasicBlock,
        symtable: &mut HashMap<String, i32>){
        let value = self.exp.convert_to_koopa_ir(func_data, entry, symtable);
        let ret = func_data.dfg_mut().new_value().ret(Some(value));
        let _ = func_data.layout_mut().bb_mut(entry).insts_mut().push_key_back(ret);
    }
}

#[derive(Debug)]
pub struct Exp{
    pub lorexp: LOrExp,
}
impl Exp {
    fn convert_to_koopa_ir(&self, func_data: &mut FunctionData, entry: BasicBlock,
        symtable: &mut HashMap<String, i32>) -> Value {
        self.lorexp.convert_to_koopa_ir(func_data, entry, symtable)
    }
    fn evaluate(&self, symtable: &HashMap<String, i32>) -> Result<i32, &'static str> {
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
        symtable: &mut HashMap<String, i32>) -> Value {
        if let Ok(val) = self.evaluate(symtable) {
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
    fn evaluate(&self, symtable: &HashMap<String, i32>) -> Result<i32, &'static str> {
        match self {
            UnaryExp::Primary(primary) => primary.evaluate(symtable),
            UnaryExp::Unary(op, unary) => {
                let val = unary.evaluate(symtable)?;
                match op {
                    UnaryOp::Pos => Ok(val),
                    UnaryOp::Neg => Ok(-val),
                    UnaryOp::Not => Ok(if val == 0 { 1 } else { 0 }),
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
        symtable: &mut HashMap<String, i32>) -> Value {
        match self {
            PrimaryExp::Exp(exp) => exp.convert_to_koopa_ir(func_data, entry, symtable),
            PrimaryExp::LVal(lval) => {
                // 查符号表获取常量值
                let val = symtable.get(&lval.ident)
                    .expect(&format!("undefined variable: {}", lval.ident));
                func_data.dfg_mut().new_value().integer(*val)
            },
            PrimaryExp::Number(n) => func_data.dfg_mut().new_value().integer(*n),
        }
    }
    fn evaluate(&self, symtable: &HashMap<String, i32>) -> Result<i32, &'static str> {
        match self {
            PrimaryExp::Exp(exp) => exp.evaluate(symtable),
            PrimaryExp::LVal(lval) => symtable.get(&lval.ident)
                .copied().ok_or("undefined variable"),
            PrimaryExp::Number(n) => Ok(*n),
        }
    }
}

#[derive(Debug)]
pub enum AddExp{
    Mul(MulExp),
    Add(Box<AddExp>, MulExp),
    Sub(Box<AddExp>, MulExp),
}
impl AddExp{
    fn convert_to_koopa_ir(&self, func_data: &mut FunctionData, entry: BasicBlock,
        symtable: &mut HashMap<String, i32>) -> Value{
        // 常量折叠
        if let Ok(val) = self.evaluate(symtable) {
            return func_data.dfg_mut().new_value().integer(val);
        }
        match self {
            AddExp::Mul(mul) => mul.convert_to_koopa_ir(func_data, entry, symtable),
            AddExp::Add(add,mul ) =>{
                let lhs = add.convert_to_koopa_ir(func_data, entry, symtable);
                let rhs = mul.convert_to_koopa_ir(func_data, entry, symtable);
                let eq = func_data.dfg_mut().new_value().binary(BinaryOp::Add, lhs, rhs);
                let _ = func_data.layout_mut().bb_mut(entry).insts_mut().push_key_back(eq);
                eq
            },
            AddExp::Sub(add,mul ) =>{
                let lhs = add.convert_to_koopa_ir(func_data, entry, symtable);
                let rhs = mul.convert_to_koopa_ir(func_data, entry, symtable);
                let eq = func_data.dfg_mut().new_value().binary(BinaryOp::Sub, lhs, rhs);
                let _ = func_data.layout_mut().bb_mut(entry).insts_mut().push_key_back(eq);
                eq
            }
        }
    }
    fn evaluate(&self, symtable: &HashMap<String, i32>) -> Result<i32, &'static str> {
        match self {
            AddExp::Mul(mul) => mul.evaluate(symtable),
            AddExp::Add(add, mul) => Ok(add.evaluate(symtable)? + mul.evaluate(symtable)?),
            AddExp::Sub(add, mul) => Ok(add.evaluate(symtable)? - mul.evaluate(symtable)?),
        }
    }
}

#[derive(Debug)]
pub enum MulExp{
    Unary(UnaryExp),
    Mul(Box<MulExp>, UnaryExp),
    Div(Box<MulExp>, UnaryExp),
    Mod(Box<MulExp>, UnaryExp),
}
impl MulExp{
    fn convert_to_koopa_ir(&self, func_data: &mut FunctionData, entry: BasicBlock,
        symtable: &mut HashMap<String, i32>) -> Value{
        if let Ok(val) = self.evaluate(symtable) {
            return func_data.dfg_mut().new_value().integer(val);
        }
        match self {
            MulExp::Unary(unaryexp) => unaryexp.convert_to_koopa_ir(func_data, entry, symtable),
            MulExp::Mul(mul, unaryexp) => {
                let lhs = mul.convert_to_koopa_ir(func_data, entry, symtable);
                let rhs = unaryexp.convert_to_koopa_ir(func_data, entry, symtable);
                let eq = func_data.dfg_mut().new_value().binary(BinaryOp::Mul, lhs, rhs);
                let _ = func_data.layout_mut().bb_mut(entry).insts_mut().push_key_back(eq);
                eq
            },
            MulExp::Div(mul, unaryexp) => {
                let lhs = mul.convert_to_koopa_ir(func_data, entry, symtable);
                let rhs = unaryexp.convert_to_koopa_ir(func_data, entry, symtable);
                let eq = func_data.dfg_mut().new_value().binary(BinaryOp::Div, lhs, rhs);
                let _ = func_data.layout_mut().bb_mut(entry).insts_mut().push_key_back(eq);
                eq
            },
            MulExp::Mod(mul, unaryexp) => {
                let lhs = mul.convert_to_koopa_ir(func_data, entry, symtable);
                let rhs = unaryexp.convert_to_koopa_ir(func_data, entry, symtable);
                let eq = func_data.dfg_mut().new_value().binary(BinaryOp::Mod, lhs, rhs);
                let _ = func_data.layout_mut().bb_mut(entry).insts_mut().push_key_back(eq);
                eq
            }
        }
    }
    fn evaluate(&self, symtable: &HashMap<String, i32>) -> Result<i32, &'static str> {
        match self {
            MulExp::Unary(unaryexp) => unaryexp.evaluate(symtable),
            MulExp::Mul(mul, unaryexp) => Ok(mul.evaluate(symtable)? * unaryexp.evaluate(symtable)?),
            MulExp::Div(mul, unaryexp) => Ok(mul.evaluate(symtable)? / unaryexp.evaluate(symtable)?),
            MulExp::Mod(mul, unaryexp) => Ok(mul.evaluate(symtable)? % unaryexp.evaluate(symtable)?),
        }
    }
}

#[derive(Debug)]
pub enum RelExp{
    Add(AddExp),
    Lt(Box<RelExp>, AddExp),
    Gt(Box<RelExp>, AddExp),
    Le(Box<RelExp>, AddExp),
    Ge(Box<RelExp>, AddExp),
}
impl RelExp{
    fn convert_to_koopa_ir(&self, func_data: &mut FunctionData, entry: BasicBlock,
        symtable: &mut HashMap<String, i32>) -> Value{
        if let Ok(val) = self.evaluate(symtable) {
            return func_data.dfg_mut().new_value().integer(val);
        }
        match self {
            RelExp::Add(add) => add.convert_to_koopa_ir(func_data, entry, symtable),
            RelExp::Lt(rel, add) => {
                let lhs = rel.convert_to_koopa_ir(func_data, entry, symtable);
                let rhs = add.convert_to_koopa_ir(func_data, entry, symtable);
                let v = func_data.dfg_mut().new_value().binary(BinaryOp::Lt, lhs, rhs);
                let _ = func_data.layout_mut().bb_mut(entry).insts_mut().push_key_back(v);
                v
            },
            RelExp::Gt(rel, add) => {
                let lhs = rel.convert_to_koopa_ir(func_data, entry, symtable);
                let rhs = add.convert_to_koopa_ir(func_data, entry, symtable);
                let v = func_data.dfg_mut().new_value().binary(BinaryOp::Gt, lhs, rhs);
                let _ = func_data.layout_mut().bb_mut(entry).insts_mut().push_key_back(v);
                v
            },
            RelExp::Le(rel, add) => {
                let lhs = rel.convert_to_koopa_ir(func_data, entry, symtable);
                let rhs = add.convert_to_koopa_ir(func_data, entry, symtable);
                let v = func_data.dfg_mut().new_value().binary(BinaryOp::Le, lhs, rhs);
                let _ = func_data.layout_mut().bb_mut(entry).insts_mut().push_key_back(v);
                v
            },
            RelExp::Ge(rel, add) => {
                let lhs = rel.convert_to_koopa_ir(func_data, entry, symtable);
                let rhs = add.convert_to_koopa_ir(func_data, entry, symtable);
                let v = func_data.dfg_mut().new_value().binary(BinaryOp::Ge, lhs, rhs);
                let _ = func_data.layout_mut().bb_mut(entry).insts_mut().push_key_back(v);
                v
            },
        }
    }
    fn evaluate(&self, symtable: &HashMap<String, i32>) -> Result<i32, &'static str> {
        match self {
            RelExp::Add(add) => add.evaluate(symtable),
            RelExp::Lt(rel, add) => Ok((rel.evaluate(symtable)? < add.evaluate(symtable)?) as i32),
            RelExp::Gt(rel, add) => Ok((rel.evaluate(symtable)? > add.evaluate(symtable)?) as i32),
            RelExp::Le(rel, add) => Ok((rel.evaluate(symtable)? <= add.evaluate(symtable)?) as i32),
            RelExp::Ge(rel, add) => Ok((rel.evaluate(symtable)? >= add.evaluate(symtable)?) as i32),
        }
    }
}

#[derive(Debug)]
pub enum EqExp{
    Rel(RelExp),
    Eq(Box<EqExp>, RelExp),
    Ne(Box<EqExp>, RelExp),
}
impl EqExp{
    fn convert_to_koopa_ir(&self, func_data: &mut FunctionData, entry: BasicBlock,
        symtable: &mut HashMap<String, i32>) -> Value{
        if let Ok(val) = self.evaluate(symtable) {
            return func_data.dfg_mut().new_value().integer(val);
        }
        match self {
            EqExp::Rel(rel) => rel.convert_to_koopa_ir(func_data, entry, symtable),
            EqExp::Eq(eq, rel) => {
                let lhs = eq.convert_to_koopa_ir(func_data, entry, symtable);
                let rhs = rel.convert_to_koopa_ir(func_data, entry, symtable);
                let v = func_data.dfg_mut().new_value().binary(BinaryOp::Eq, lhs, rhs);
                let _ = func_data.layout_mut().bb_mut(entry).insts_mut().push_key_back(v);
                v
            },
            EqExp::Ne(eq, rel) => {
                let lhs = eq.convert_to_koopa_ir(func_data, entry, symtable);
                let rhs = rel.convert_to_koopa_ir(func_data, entry, symtable);
                let v = func_data.dfg_mut().new_value().binary(BinaryOp::NotEq, lhs, rhs);
                let _ = func_data.layout_mut().bb_mut(entry).insts_mut().push_key_back(v);
                v
            },
        }
    }
    fn evaluate(&self, symtable: &HashMap<String, i32>) -> Result<i32, &'static str> {
        match self {
            EqExp::Rel(rel) => rel.evaluate(symtable),
            EqExp::Eq(eq, rel) => Ok((eq.evaluate(symtable)? == rel.evaluate(symtable)?) as i32),
            EqExp::Ne(eq, rel) => Ok((eq.evaluate(symtable)? != rel.evaluate(symtable)?) as i32),
        }
    }
}

#[derive(Debug)]
pub enum LAndExp{
    Eq(EqExp),
    And(Box<LAndExp>, EqExp),
}
impl LAndExp{
    fn convert_to_koopa_ir(&self, func_data: &mut FunctionData, entry: BasicBlock,
        symtable: &mut HashMap<String, i32>) -> Value{
        if let Ok(val) = self.evaluate(symtable) {
            return func_data.dfg_mut().new_value().integer(val);
        }
        match self {
            LAndExp::Eq(eq) => eq.convert_to_koopa_ir(func_data, entry, symtable),
            LAndExp::And(land, eq) => {
                let lhs = land.convert_to_koopa_ir(func_data, entry, symtable);
                let rhs = eq.convert_to_koopa_ir(func_data, entry, symtable);
                let v = func_data.dfg_mut().new_value().binary(BinaryOp::And, lhs, rhs);
                let _ = func_data.layout_mut().bb_mut(entry).insts_mut().push_key_back(v);
                v
            },
        }
    }
    fn evaluate(&self, symtable: &HashMap<String, i32>) -> Result<i32, &'static str> {
        match self {
            LAndExp::Eq(eq) => eq.evaluate(symtable),
            LAndExp::And(land, eq) => {
                let l = land.evaluate(symtable)?;
                let r = eq.evaluate(symtable)?;
                Ok(if l != 0 && r != 0 { 1 } else { 0 })
            }
        }
    }
}

#[derive(Debug)]
pub enum LOrExp{
    And(LAndExp),
    Or(Box<LOrExp>, LAndExp),
}
impl LOrExp{
    fn convert_to_koopa_ir(&self, func_data: &mut FunctionData, entry: BasicBlock,
        symtable: &mut HashMap<String, i32>) -> Value{
        if let Ok(val) = self.evaluate(symtable) {
            return func_data.dfg_mut().new_value().integer(val);
        }
        match self {
            LOrExp::And(land) => land.convert_to_koopa_ir(func_data, entry, symtable),
            LOrExp::Or(lor, land) => {
                let lhs = lor.convert_to_koopa_ir(func_data, entry, symtable);
                let rhs = land.convert_to_koopa_ir(func_data, entry, symtable);
                let v = func_data.dfg_mut().new_value().binary(BinaryOp::Or, lhs, rhs);
                let _ = func_data.layout_mut().bb_mut(entry).insts_mut().push_key_back(v);
                v
            },
        }
    }
    fn evaluate(&self, symtable: &HashMap<String, i32>) -> Result<i32, &'static str> {
        match self {
            LOrExp::And(land) => land.evaluate(symtable),
            LOrExp::Or(lor, land) => {
                let l = lor.evaluate(symtable)?;
                let r = land.evaluate(symtable)?;
                Ok(if l != 0 || r != 0 { 1 } else { 0 })
            }
        }
    }
}

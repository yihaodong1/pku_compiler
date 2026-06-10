use koopa::ir::*;
use koopa::ir::builder_traits::*;

#[derive(Debug)]
pub struct CompUnit {
  pub func_def: FuncDef,
}
impl CompUnit {
    pub fn convert_to_koopa_ir(&self) -> Program {
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
}
impl FuncDef {
    fn convert_to_koopa_ir(&self, program: &mut Program){
        match self.func_type {
            FuncType::Void => {
                let name = format!("@{}", self.ident);
                let func = program.new_func_def(name, vec![], Type::get_i32());
                let func_data = program.func_mut(func);
                self.block.convert_to_koopa_ir(func_data);
            },
            FuncType::Int => {
                let name = format!("@{}", self.ident);
                let func = program.new_func_def(name, vec![], Type::get_i32());
                let func_data = program.func_mut(func);
                self.block.convert_to_koopa_ir(func_data);
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
  pub stmt: Stmt,
}
impl Block {
    fn convert_to_koopa_ir(&self, func_data: &mut FunctionData){
        let entry = func_data.dfg_mut().new_bb().basic_block(Some("%entry".into()));
        let _ = func_data.layout_mut().bbs_mut().push_key_back(entry);
        self.stmt.convert_to_koopa_ir(func_data, entry);
    }
}

#[derive(Debug)]
pub struct Stmt {
  pub exp: Exp,
}
impl Stmt {
    fn convert_to_koopa_ir(&self, func_data: &mut FunctionData, entry: koopa::ir::BasicBlock){
        let value = self.exp.convert_to_koopa_ir(func_data, entry);
        let ret = func_data.dfg_mut().new_value().ret(Some(value));
        let _ = func_data.layout_mut().bb_mut(entry).insts_mut().push_key_back(ret);
    }
}

#[derive(Debug)]
pub struct Exp{
    pub addexp: AddExp,
}
impl Exp {
    fn convert_to_koopa_ir(&self, func_data: &mut FunctionData, entry: BasicBlock) -> Value {
        self.addexp.convert_to_koopa_ir(func_data, entry)
    }
}

#[derive(Debug)]
pub enum UnaryExp{
    Primary(PrimaryExp),
    Unary(UnaryOp, Box<UnaryExp>),
}
impl UnaryExp {
    fn convert_to_koopa_ir(&self, func_data: &mut FunctionData, entry: BasicBlock) -> Value {
        match self {
            UnaryExp::Primary(primary) => primary.convert_to_koopa_ir(func_data, entry),
            UnaryExp::Unary(op, unary) => {
                let val = unary.convert_to_koopa_ir(func_data, entry);
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
}

#[derive(Debug)]
pub enum UnaryOp{
    Pos,
    Neg,
    Not
}

#[derive(Debug)]
pub enum PrimaryExp{
    Exp(Box<Exp>),
    Number(i32),
}
impl PrimaryExp {
    fn convert_to_koopa_ir(&self, func_data: &mut FunctionData, entry: BasicBlock) -> Value {
        match self {
            PrimaryExp::Exp(exp) => exp.convert_to_koopa_ir(func_data, entry),
            PrimaryExp::Number(n) => func_data.dfg_mut().new_value().integer(*n),
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
    fn convert_to_koopa_ir(&self, func_data: &mut FunctionData, entry: BasicBlock) -> Value{
        match self {
            AddExp::Mul(mul) => mul.convert_to_koopa_ir(func_data, entry),
            AddExp::Add(add,mul ) =>{
                let lhs = add.convert_to_koopa_ir(func_data, entry);
                let rhs = mul.convert_to_koopa_ir(func_data, entry);
                let eq = func_data.dfg_mut().new_value().binary(BinaryOp::Add, lhs, rhs);
                let _ = func_data.layout_mut().bb_mut(entry).insts_mut().push_key_back(eq);
                eq
            },
            AddExp::Sub(add,mul ) =>{
                let lhs = add.convert_to_koopa_ir(func_data, entry);
                let rhs = mul.convert_to_koopa_ir(func_data, entry);
                let eq = func_data.dfg_mut().new_value().binary(BinaryOp::Sub, lhs, rhs);
                let _ = func_data.layout_mut().bb_mut(entry).insts_mut().push_key_back(eq);
                eq
            }
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
    fn convert_to_koopa_ir(&self, func_data: &mut FunctionData, entry: BasicBlock) -> Value{
        match self {
            MulExp::Unary(unaryexp) => unaryexp.convert_to_koopa_ir(func_data, entry),
            MulExp::Mul(mul, unaryexp) => {
                let lhs = mul.convert_to_koopa_ir(func_data, entry);
                let rhs = unaryexp.convert_to_koopa_ir(func_data, entry);
                let eq = func_data.dfg_mut().new_value().binary(BinaryOp::Mul, lhs, rhs);
                let _ = func_data.layout_mut().bb_mut(entry).insts_mut().push_key_back(eq);
                eq
            },
            MulExp::Div(mul, unaryexp) => {
                let lhs = mul.convert_to_koopa_ir(func_data, entry);
                let rhs = unaryexp.convert_to_koopa_ir(func_data, entry);
                let eq = func_data.dfg_mut().new_value().binary(BinaryOp::Div, lhs, rhs);
                let _ = func_data.layout_mut().bb_mut(entry).insts_mut().push_key_back(eq);
                eq
            },
            MulExp::Mod(mul, unaryexp) => {
                let lhs = mul.convert_to_koopa_ir(func_data, entry);
                let rhs = unaryexp.convert_to_koopa_ir(func_data, entry);
                let eq = func_data.dfg_mut().new_value().binary(BinaryOp::Mod, lhs, rhs);
                let _ = func_data.layout_mut().bb_mut(entry).insts_mut().push_key_back(eq);
                eq
            }
        }
    }
}

// ...

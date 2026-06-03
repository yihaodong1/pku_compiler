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
  pub num: i32,
}
impl Stmt {
    fn convert_to_koopa_ir(&self, func_data: &mut FunctionData, entry: koopa::ir::BasicBlock){
        let num = func_data.dfg_mut().new_value().integer(self.num);
        let ret = func_data.dfg_mut().new_value().ret(Some(num));
        let _ = func_data.layout_mut().bb_mut(entry).insts_mut().push_key_back(ret);
    }
}
// ...

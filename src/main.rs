mod ast;
use lalrpop_util::lalrpop_mod;
use std::env::args;
use std::fs::read_to_string;
use std::fs;
use std::io::Result;
use koopa::ir::{*, builder_traits::*};
use koopa::back::KoopaGenerator;
// 引用 lalrpop 生成的解析器
// 因为我们刚刚创建了 sysy.lalrpop, 所以模块名是 sysy
lalrpop_mod!(sysy);
// 根据内存形式 Koopa IR 生成汇编
trait GenerateAsm {
  fn generate(&self, buf:&mut String);
}

impl GenerateAsm for koopa::ir::Program {
  fn generate(&self, buf:&mut String) {
    buf.push_str(" .text\n");
    for &func in self.func_layout() {
      self.func(func).generate(buf);
    }
  }
}

impl GenerateAsm for koopa::ir::FunctionData {
  fn generate(&self, buf:&mut String) {
    buf.push_str(" .global ");
    let fun_name = String::from(self.name());
    buf.push_str(&fun_name[1..]);
    buf.push_str("\n");
    buf.push_str(&fun_name[1..]);
    buf.push_str(":\n");
    for bb_node in self.layout().bbs().nodes() {
      for (&value, _) in bb_node.insts() {
        let value_data = self.dfg().value(value);
        generate_value(value, value_data, self, buf);
      }
    }
  }
}

fn generate_value(
  value: Value,
  value_data: &entities::ValueData,
  func: &FunctionData,
  buf: &mut String,
) {
  match value_data.kind() {
    ValueKind::Return(ret) => {
      let ret_value = ret.value();
      if let Some(ret_value) = ret_value {
        buf.push_str(" li a0, ");
        let ret_value_data = func.dfg().value(ret_value);
        match ret_value_data.kind() {
          ValueKind::Integer(i) => {
            buf.push_str(&i.value().to_string());
          }
          _ => {}
        }
        buf.push_str("\n");
      }
      buf.push_str(" ret\n");
    }
    _ => {}
  }
}

fn main() -> Result<()> {
  // 解析命令行参数
  let mut args = args();
  args.next();
  let mode = args.next().unwrap();
  let input = args.next().unwrap();
  args.next();
  let output = args.next().unwrap();

  // 读取输入文件
  let input = read_to_string(input)?;

  // 调用 lalrpop 生成的 parser 解析输入文件
  let ast = sysy::CompUnitParser::new().parse(&input).unwrap();

  // 输出解析得到的 AST
  // println!("{:#?}", ast);
  let program = ast.convert_to_koopa_ir();
  let mut g = KoopaGenerator::new(Vec::new());
  g.generate_on(&program).unwrap();
  // println!("{}", text_form_ir);
  match mode.as_str(){
    "-koopa"=>{
      let text_form_ir = std::str::from_utf8(&g.writer()).unwrap().to_string();
      fs::write(output, text_form_ir)?
    },
    "-riscv"=>{
      let mut buf = String::new();
      program.generate(&mut buf);
      println!("{}", buf);
      fs::write(output, buf)?
    },
    _=>{}
  }
  Ok(())
}

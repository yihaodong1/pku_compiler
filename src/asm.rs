use koopa::ir::{*, entities};

/// 根据内存形式 Koopa IR 生成 RISC-V 汇编
pub trait GenerateAsm {
  fn generate(&self, buf: &mut String);
}

impl GenerateAsm for Program {
  fn generate(&self, buf: &mut String) {
    buf.push_str(" .text\n");
    for &func in self.func_layout() {
      self.func(func).generate(buf);
    }
  }
}

impl GenerateAsm for FunctionData {
  fn generate(&self, buf: &mut String) {
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

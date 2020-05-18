// FIXME: Make me pass! Diff budget: 30 lines.

#[derive(Default)]
struct Builder {
    string: Option<String>,
    number: Option<usize>,
}

impl Builder {
    // fn string(...
    fn string<T: Into<String>>(mut self, s : T) -> Self {
      self.string = Some(s.into());
      self
    }

    // fn number(...
    fn number(mut self, n : usize) -> Self {
      self.number = Some(n);
      self
    }
}

impl ToString for Builder {
    // Implement the trait
    fn to_string(&self) -> String {
      let s : String = match &self.string {
        None => String::from(""),
        Some(s) => s.to_string()
      };

      let n : String = match self.number {
        None => String::from(""),
        Some(n) => n.to_string()
      };

      
      String::from(format!("{} {}", s, n).trim())
    }
}

// Do not modify this function.
#[test]
fn builder() {
    let empty = Builder::default().to_string();
    assert_eq!(empty, "");

    let just_str = Builder::default().string("hi").to_string();
    assert_eq!(just_str, "hi");

    let just_num = Builder::default().number(254).to_string();
    assert_eq!(just_num, "254");

    let a = Builder::default()
        .string("hello, world!")
        .number(200)
        .to_string();

    assert_eq!(a, "hello, world! 200");

    let b = Builder::default()
        .string("hello, world!")
        .number(200)
        .string("bye now!")
        .to_string();

    assert_eq!(b, "bye now! 200");

    let c = Builder::default().string("heap!".to_owned()).to_string();

    assert_eq!(c, "heap!");
}

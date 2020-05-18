use core::panic::PanicInfo;

use crate::console::{kprintln};

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
  let ascii_art ="
            (
       (      )     )
         )   (    (
        (          `
    .-\"\"^\"\"\"^\"\"^\"\"\"^\"\"-.
  (//\\\\//\\\\//\\\\//\\\\//\\\\//)
   ~\\^^^^^^^^^^^^^^^^^^/~
     `================`

     The pi is overdone.";

  kprintln!("{}", ascii_art);

  kprintln!("\n---------- PANIC ----------\n");

  match info.location() {
    Some(location) => {
      kprintln!("FILE: {}", location.file());
      kprintln!("LINE: {}", location.line());
      kprintln!("COL:  {}", location.column());

    },
    None => {}
  }

  kprintln!("");

  kprintln!("Panic occurred: {:#?}", info);

  loop {}
}

use coffret_device::{recovery_code, RecoveryCode};

use crate::answer::{self, Answer, Form, Ran};
use crate::library_args::LibraryArgs;
use coffret_shell::passphrase;

pub fn run(args: LibraryArgs, form: Form) -> anyhow::Result<Ran> {
    let code = recovery_code(&args.library, passphrase::entering(args.passphrase_stdin))?;
    print_recovery_code(&code, form);
    Ok(Ran::clean(Answer::RecoveryCode(answer::RecoveryCode::new(
        args.library,
        &code,
    ))))
}

/// Prints the code, and says plainly what it is for.
///
/// The code goes to standard output, in the grouped form it is meant to be
/// copied from; everything around it goes to standard error, so that a caller
/// piping this command gets the code and nothing else.
///
/// The warning is not decoration. The Master Key is the only thing that opens
/// the Library, coffret keeps no copy of it anywhere, and a Storage provider
/// holds nothing but ciphertext — so losing every device that holds the Library
/// and every Recovery Code written down for it leaves it unreadable by anyone,
/// permanently.
///
/// Under `--json` the code is in the answer rather than on a line of its own,
/// and the warning is printed all the same: it is for the person, whatever
/// form the script that ran the command reads. Since "write this down" is then
/// followed by no code on the terminal, it says where the code went instead,
/// so that the person watching knows to take it from the answer.
pub fn print_recovery_code(code: &RecoveryCode, form: Form) {
    eprintln!("\nRecovery Code — write this down and keep it away from this device.");
    eprintln!("It is the only copy of this Library's Master Key that exists off the");
    eprintln!("device. If every device holding the Library is lost and no Recovery");
    eprintln!("Code was kept, nothing and nobody can open the Library again.\n");
    match form {
        Form::Text => println!("{}", code.to_grouped_string()),
        Form::Json => eprintln!("It is recovery_code in the answer on standard output."),
    }
}

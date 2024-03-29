#![forbid(unsafe_code)]
// https://github.com/clap-rs/clap
use clap::{App, AppSettings, Arg, ArgGroup, ArgSettings};

pub fn main() {
  // This example shows how to create an application with several arguments using usage strings, which can be
  // far less verbose that shown in 01b_QuickExample.rs, but is more readable. The downside is you cannot set
  // the more advanced configuration options using this method (well...actually you can, you'll see ;) )
  //
  // The example below is functionally identical to the 01b_quick_example.rs and 01c_quick_example.rs
  //
  // Create an application with 5 possible arguments (2 auto generated) and 2 subcommands (1 auto generated)
  //    - A config file
  //        + Uses "-c filename" or "--config filename"
  //    - An output file
  //        + A positional argument (i.e. "$ myapp output_filename")
  //    - A debug flag
  //        + Uses "-d" or "--debug"
  //        + Allows multiple occurrences of such as "-dd" (for vary levels of debugging, as an example)
  //    - A help flag (automatically generated by clap)
  //        + Uses "-h" or "--help" (Only autogenerated if you do NOT specify your own "-h" or "--help")
  //    - A version flag (automatically generated by clap)
  //        + Uses "-V" or "--version" (Only autogenerated if you do NOT specify your own "-V" or "--version")
  //    - A subcommand "test" (subcommands behave like their own apps, with their own arguments
  //        + Used by "$ myapp test" with the following arguments
  //            > A list flag
  //                = Uses "-l" (usage is "$ myapp test -l"
  //            > A help flag (automatically generated by clap
  //                = Uses "-h" or "--help" (full usage "$ myapp test -h" or "$ myapp test --help")
  //            > A version flag (automatically generated by clap
  //                = Uses "-V" or "--version" (full usage "$ myapp test -V" or "$ myapp test --version")
  //    - A subcommand "help" (automatically generated by clap because we specified a subcommand of our own)
  //        + Used by "$ myapp help" (same functionality as "-h" or "--help")

  let matches = App::new("Galera CLI")
    .version("0.1")
    .author("Ondřej Pešek <iTzBoboCz@users.noreply.github.com>")
    .about("Does awesome things")
    .setting(AppSettings::ArgRequiredElseHelp)
    // od 3.0.0-beta.3
    // .license(crate_license!())
    // .license("MIT OR Apache-2.0")
    .subcommand(
      App::new("scan")
        .about("scans new media")
        .setting(AppSettings::ArgRequiredElseHelp)
        // add group so that only all or u can be passed
        .group(
          ArgGroup::new("scan")
          .args(&["all", "users"])
          .multiple(false)
          .required(true),
        )
        .arg(
          Arg::new("all")
            .about("scans media of all users")
            .short('a')
            .long("all")
            .takes_value(false),
        )
        .arg(
          Arg::new("users")
            // add subcommand placeholder
            .about("scans media of specified users")
            .short('u')
            .long("users")
            .takes_value(true)
            .multiple(true),
        ),
    )
    .get_matches();

  // You can check the value provided by positional arguments, or option arguments
  if let Some(o) = matches.value_of("users") {
    println!("Value for output: {}", o);
  }

  if let Some(c) = matches.value_of("config") {
    println!("Value for config: {}", c);
  }

  // You can see how many times a particular flag or argument occurred
  // Note, only flags can have multiple occurrences
  match matches.occurrences_of("debug") {
    0 => println!("Debug mode is off"),
    1 => println!("Debug mode is kind of on"),
    2 => println!("Debug mode is on"),
    _ => println!("Don't be crazy"),
  }

  // You can check for the existence of subcommands, and if found use their
  // matches just as you would the top level app
  if let Some(matches) = matches.subcommand_matches("scan") {
    if matches.is_present("users") {
      let u: Vec<&str> = matches.values_of("users").unwrap().collect();
      println!("users: {:?}", u);
    } else if matches.is_present("all") {
      println!("all users");
    } else {
      println!("Not printing testing lists...");

    }
  }

  // You can check for the existence of subcommands, and if found use their
  // matches just as you would the top level app
  if let Some(matches) = matches.subcommand_matches("secret") {
    // "$ myapp test" was run
    if matches.is_present("list") {
      // "$ myapp test -l" was run
      println!("Printing testing lists...");
    } else {
      println!("Not printing testing lists...");
    }
  }

  // Continued program logic goes here...
}


/// Scans for new media
/// # Example
/// ```
/// // Some<Vec<&str>> is a list of users
/// let vec = vec!("foo", "bar")
/// scan(Some(vec))
///
/// // None means all users
/// scan(None)
/// ```
pub async fn scan(users: Option<Vec<&str>>) {
  // request na api?
  // galera::scan::scan_root(&conn, xdg_data, user_id).await;
}

// PROBLÉM:
// request na api: verifikační token, funguje jen když funguje server (když spadne, tak nic nemůžeme dělat)
// nebo
// velký galera-cli soubor, protože by musel mít vlastní přípojení na databázi

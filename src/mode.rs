//! Handles run modes.

type App = clap::App<'static, 'static>;
type Arg = clap::Arg<'static, 'static>;
type Matches = clap::ArgMatches<'static>;

/// Run modes.
#[derive(Debug, Clone)]
pub enum Mode {
    /// Check mode, attempt to prove the `input` system is correct.
    Check {
        input: String,
        smt_log: Option<String>,
        induction: bool,
        bmc: bool,
        bmc_max: Option<usize>,
    },
    /// Demo mode, generate a demo system to `target`.
    Demo { target: String },
    /// Parse mode, does nothing but parse the system.
    Parse { input: String },
}

impl Mode {
    /// Yields all the mode subcommands.
    pub fn subcommands() -> Vec<App> {
        vec![
            cla::check_subcommand(),
            cla::demo_subcommand(),
            cla::bmc_subcommand(),
            cla::parse_subcommand(),
        ]
    }

    /// Builds itself from top-level clap matches.
    pub fn from_clap(smt_log: Option<String>, matches: &Matches) -> Option<Mode> {
        let modes = [cla::try_check, cla::try_bmc, cla::try_demo, cla::try_parse];
        for try_mode in &modes {
            let maybe_res = try_mode(smt_log.clone(), matches);
            if maybe_res.is_some() {
                return maybe_res;
            }
        }
        None
    }
}

pub mod cla {
    use super::*;
    use clap::SubCommand;

    pub mod mode {
        pub const CHECK: &str = "check";
        pub const DEMO: &str = "demo";
        pub const BMC: &str = "bmc";
        pub const PARSE: &str = "parse";
    }

    mod arg {
        pub const BMC_KEY: &str = "BMC";
        pub const BMC_MAX_KEY: &str = "BMC_MAX";
        pub const SMT_LOG_KEY: &str = "SMT_LOG";
        pub const SYS_KEY: &str = "SYS_KEY";
        pub const DEMO_TGT_KEY: &str = "DEMO_TGT";
    }

    fn bmc_max_arg() -> Arg {
        Arg::with_name(arg::BMC_MAX_KEY)
            .help(
                "Maximum number of transitions ≥ 0 allowed from the \
                initial state(s) in BMC, infinite by default",
            )
            .long("bmc_max")
            .validator(validate_int)
            .value_name("INT")
    }
    /// Yields the BMC max value, if any.
    fn get_bmc_max(matches: &Matches, mut if_present_do: impl FnMut()) -> Option<usize> {
        matches.value_of(arg::BMC_MAX_KEY).map(|val| {
            if_present_do();
            usize::from_str_radix(val, 10)
                .expect(&format!("[clap] unexpected value for BMC max: `{}`", val))
        })
    }

    pub fn smt_log_arg() -> Arg {
        Arg::with_name(arg::SMT_LOG_KEY)
            .help("Activates SMT logging in the directory specified")
            .long("smt_log")
            .short("l")
            .value_name("DIR")
    }
    pub fn get_smt_log(matches: &Matches) -> Option<String> {
        matches.value_of(arg::SMT_LOG_KEY).map(String::from)
    }

    fn sys_arg() -> Arg {
        Arg::with_name(arg::SYS_KEY)
            .help("Transition system to analyze (run in demo mode for details)")
            .required(true)
            .value_name("FILE")
    }
    fn get_sys(matches: &Matches) -> String {
        matches
            .value_of(arg::SYS_KEY)
            .expect("[clap] required argument cannot be absent")
            .into()
    }

    /// Subcommand for the check mode.
    pub fn check_subcommand() -> App {
        SubCommand::with_name(mode::CHECK)
            .about("Attempts to prove that the input transition system is correct")
            .args(&[
                Arg::with_name(arg::BMC_KEY)
                    .help(
                        "Activates BMC (Bounded Model-Checking): \
                        looks for a falsification for POs found to not be inductive",
                    )
                    .long("bmc"),
                bmc_max_arg(),
                smt_log_arg(),
                sys_arg(),
            ])
    }
    pub fn try_check(smt_log: Option<String>, matches: &Matches) -> Option<Mode> {
        let matches = matches.subcommand_matches(mode::CHECK)?;

        let input = get_sys(matches);
        let smt_log = get_smt_log(matches).or(smt_log);

        let mut bmc = matches.is_present(arg::BMC_KEY);
        let bmc_max = get_bmc_max(matches, || bmc = true);

        Some(Mode::Check {
            input,
            smt_log,
            induction: true,
            bmc,
            bmc_max,
        })
    }

    /// Subcommand for the demo mode.
    pub fn demo_subcommand() -> App {
        SubCommand::with_name(mode::DEMO)
            .about(
                "Generates a demo transition system file, \
                recommended if you are just starting out. \
                /!\\ OVERWRITES the target file.",
            )
            .arg(
                Arg::with_name(arg::DEMO_TGT_KEY)
                    .help("Path of the file to write the demo file to")
                    .required(true),
            )
    }
    pub fn try_demo(_smt_log: Option<String>, matches: &Matches) -> Option<Mode> {
        let matches = matches.subcommand_matches(mode::DEMO)?;
        let target = matches
            .value_of(arg::DEMO_TGT_KEY)
            .expect("[clap]: required argument cannot be absent")
            .into();
        Some(Mode::Demo { target })
    }

    /// Subcommand for the bmc mode.
    pub fn bmc_subcommand() -> App {
        SubCommand::with_name(mode::BMC)
            .about(
                "Runs BMC (Bounded Model Checking) without induction. \
            Mikino will search for a falsification for each proof objective.",
            )
            .args(&[bmc_max_arg(), smt_log_arg(), sys_arg()])
    }
    pub fn try_bmc(smt_log: Option<String>, matches: &Matches) -> Option<Mode> {
        let matches = matches.subcommand_matches(mode::BMC)?;
        let bmc_max = get_bmc_max(matches, || ());
        let smt_log = get_smt_log(matches).or(smt_log);
        let input = get_sys(matches);
        let induction = false;
        let bmc = true;
        Some(Mode::Check {
            input,
            bmc,
            bmc_max,
            induction,
            smt_log,
        })
    }

    /// Subcommand for parse mode.
    pub fn parse_subcommand() -> App {
        SubCommand::with_name(mode::PARSE)
            .about("Parses the input system and exits")
            .arg(sys_arg())
    }
    pub fn try_parse(_smt_log: Option<String>, matches: &Matches) -> Option<Mode> {
        let matches = matches.subcommand_matches(mode::PARSE)?;
        let input = get_sys(matches);
        Some(Mode::Parse { input })
    }

    /// Returns an error if the input string is not a valid integer.
    ///
    /// Used by CLAP.
    pub fn validate_int(s: String) -> Result<(), String> {
        macro_rules! abort {
            () => {
                return Err(format!("expected integer, found `{}`", s));
            };
        }
        if s != "0" {
            for (idx, char) in s.chars().enumerate() {
                if idx == 0 {
                    if !char.is_numeric() || char == '0' {
                        abort!()
                    }
                } else {
                    if !char.is_numeric() {
                        abort!()
                    }
                }
            }
        }
        Ok(())
    }
}

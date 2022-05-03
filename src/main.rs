#![allow(dead_code)]

mikino_api::prelude!();

use std::{collections::BTreeSet as Set, io::Write, ops::Deref, path::PathBuf};

use check::{BaseRes, CheckRes, StepRes};
use trans::Sys;

use ansi_term::{Colour, Style};

#[macro_export]
macro_rules! prelude {
    {} => { use crate::prelude::*; };
    { pub } => { pub use crate::prelude::*; };
}

pub mod mode;

use mode::Mode;

/// Entry point.
pub fn main() {
    Run::new().launch()
}

/// Post-run structure.
pub struct PostRun<'sys> {
    pub env: Run,
    pub base: BaseRes<'sys>,
    pub step: StepRes<'sys>,
}

/// Run environment.
pub struct Run {
    /// Output styles (for coloring).
    pub styles: Styles,
    /// Verbosity.
    pub verb: usize,
    /// Z3 command.
    pub z3_cmd: String,
    /// Run mode.
    pub mode: Mode,
}
impl Deref for Run {
    type Target = Styles;
    fn deref(&self) -> &Styles {
        &self.styles
    }
}
impl Run {
    /// Constructor, handles CLAP.
    pub fn new() -> Self {
        use clap::*;
        let app = clap::Command::new("mikino")
            .version(crate_version!())
            .author(crate_authors!())
            .about(
                "A minimal induction engine for transition systems. \
                See the `demo` subcommand if you are just starting out.",
            )
            .args(&[
                Arg::new("NO_COLOR")
                    .long("no_color")
                    .help("Deactivates colored output"),
                Arg::new("VERB")
                    .short('v')
                    .multiple_occurrences(true)
                    .help("Increases verbosity"),
                Arg::new("Z3_CMD")
                    .long("z3_cmd")
                    .takes_value(true)
                    .default_value("z3")
                    .help("specifies the command to run Z3"),
                Arg::new("QUIET")
                    .short('q')
                    .help("Quiet output, only shows the final result (/!\\ hides counterexamples)"),
                mode::cla::smt_log_arg(),
            ])
            .subcommands(mode::Mode::subcommands())
            .subcommand_required(true)
            .color(clap::ColorChoice::Auto);

        let matches = app.get_matches();
        let color = matches.occurrences_of("NO_COLOR") == 0;
        let verb = ((matches.occurrences_of("VERB") + 1) % 4) as usize;
        let quiet = matches.occurrences_of("QUIET") > 0;
        let z3_cmd = matches
            .value_of("Z3_CMD")
            .expect("argument with default value")
            .into();
        let smt_log = mode::cla::get_smt_log(&matches);
        let verb = if quiet {
            0
        } else if verb > 4 {
            4
        } else {
            verb
        };

        let mode =
            mode::Mode::from_clap(smt_log, &matches).expect("[clap] could not recognize mode");

        Self {
            styles: Styles::new(color),
            verb,
            z3_cmd,
            mode,
        }
    }

    /// Launches whatever the user told us to do.
    pub fn launch(&self) {
        if let Err(e) = self.run() {
            println!("|===| {}", self.red.paint("Error"));
            for (e_idx, e) in e.into_iter().enumerate() {
                for (l_idx, line) in e.pretty(&self.styles).lines().enumerate() {
                    let pref = if e_idx == 0 {
                        "| "
                    } else if l_idx == 0 {
                        "| - "
                    } else {
                        "|   "
                    };
                    println!("{}{}", pref, line);
                }
            }
            println!("|===|");
        }
    }

    /// Runs the mode.
    pub fn run(&self) -> Res<()> {
        match &self.mode {
            Mode::Check {
                input,
                smt_log,
                induction,
                bmc,
                bmc_max,
            } => {
                if let Some(smt_log) = smt_log {
                    if !std::path::Path::new(smt_log).exists() {
                        std::fs::create_dir_all(smt_log).chain_err(|| {
                            format!("while recursively creating SMT log directory `{}`", smt_log)
                        })?
                    }
                }
                let check = Check::new(self, input, smt_log)?;
                let (base, step) = if *induction {
                    let (base, step) = check.run()?;
                    (base, Some(step))
                } else {
                    (CheckRes::new(&check.sys).into(), None)
                };
                if *bmc {
                    if *induction {
                        println!();
                    }
                    check.bmc(bmc_max.clone(), &base, step.as_ref())?
                }
                Ok(())
            }
            Mode::Script {
                input,
                smt_log,
                verb,
            } => {
                if let Some(smt_log) = smt_log {
                    if !std::path::Path::new(smt_log).exists() {
                        std::fs::create_dir_all(smt_log).chain_err(|| {
                            format!("while recursively creating SMT log directory `{}`", smt_log)
                        })?
                    }
                }

                run_script(self, input, smt_log, *verb)
                    .chain_err(|| format!("running `{}` script", self.styles.bold.paint(input)))
            }
            Mode::Demo { target, check } => self.write_demo(target, *check),
            Mode::Parse { input } => {
                let _check = Check::new(self, input, &None)?;
                Ok(())
            }
        }
    }

    /// Writes the demo system file somewhere.
    ///
    /// If `!check`, generates the demo script instead.
    pub fn write_demo(&self, target: &str, check: bool) -> Res<()> {
        use std::fs::OpenOptions;
        let (desc, demo) = if check {
            ("system", mikino_api::TRANS_DEMO.as_bytes())
        } else {
            ("script", mikino_api::SCRIPT_DEMO.as_bytes())
        };
        println!(
            "writing demo {} to file `{}`",
            desc,
            self.bold.paint(target)
        );
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(target)
            .chain_err(|| format!("while opening file `{}` in write mode", target))?;
        file.write(demo)
            .chain_err(|| format!("while writing demo {} to file `{}`", desc, target))?;
        file.flush()
            .chain_err(|| format!("while writing demo {} to file `{}`", desc, target))?;
        Ok(())
    }
}

/// Runs a script.
pub fn run_script(
    env: &Run,
    script_path: impl AsRef<std::path::Path>,
    smt_log_dir: &Option<String>,
    verb: usize,
) -> Res<()> {
    let with_pos = verb > 0;
    let script_path = script_path.as_ref();
    let script_content = {
        use std::{fs::OpenOptions, io::Read};
        let mut file = OpenOptions::new()
            .read(true)
            .open(script_path)
            .chain_err(|| {
                format!(
                    "loading file `{}`",
                    env.bold.paint(script_path.display().to_string())
                )
            })?;

        let mut content = String::new();
        file.read_to_string(&mut content).chain_err(|| {
            format!(
                "reading file `{}`",
                env.bold.paint(script_path.display().to_string())
            )
        })?;
        content
    };

    let ast = parse::script(&script_content).chain_err(|| {
        format!(
            "parsing (1) file `{}`",
            env.bold.paint(script_path.display().to_string())
        )
    })?;
    let script = script::build::doit(ast)
        .map_err(|e| {
            let span = e.span;
            let (prev, row, col, line, next) = span.pretty_of(&script_content);
            Error::parse("", row, col, line, prev, next).extend(e.error.into_iter())
        })
        .chain_err(|| {
            format!(
                "parsing (2) file `{}`",
                env.bold.paint(script_path.display().to_string())
            )
        })?;
    if env.verb >= 3 {
        println!("parsing {}", env.styles.green.paint("successful"));
    }

    let mut runner = {
        let conf = SmtConf::z3(&env.z3_cmd);
        let tee = smt_log_dir.as_ref().map(|s| {
            let mut path = PathBuf::from(s);
            path.push("script.smt2");
            path
        });
        mikino_api::script::Script::new(conf, tee, &script, &script_content).chain_err(|| {
            format!(
                "building script runner for file `{}`",
                env.bold.paint(script_path.display().to_string())
            )
        })?
    };

    'step: loop {
        use mikino_api::script::{Outcome, Step};
        match runner.step().chain_err(|| {
            format!(
                "performing script step for file `{}`",
                env.bold.paint(script_path.display().to_string())
            )
        })? {
            Step::Done(Outcome::Exit(_span_opt, code)) => {
                println!(
                    "{}",
                    Outcome::Exit(_span_opt, code).pretty(&script_content, &env.styles, with_pos)
                );
                if code != 0 {
                    std::process::exit(code as i32)
                } else {
                    break 'step;
                }
            }
            Step::Done(outcome @ Outcome::Panic { .. }) => {
                eprintln!("{}", outcome.pretty(&script_content, &env.styles, with_pos));
                bail!(
                    "script `{}` panicked",
                    env.bold.paint(script_path.display().to_string())
                )
            }
            step => {
                if let Some(pretty) = step.pretty(&script_content, &env.styles, with_pos) {
                    println!("{}", pretty)
                }
            }
        }
    }

    Ok(())
}

/// Check environment.
pub struct Check<'env> {
    /// Run env.
    pub env: &'env Run,
    /// System to check.
    pub sys: Sys,
    /// Optional SMT log directory.
    pub smt_log_dir: Option<String>,
}
impl<'env> Deref for Check<'env> {
    type Target = Styles;
    fn deref(&self) -> &Styles {
        self.env.deref()
    }
}
impl<'env> Check<'env> {
    /// Constructor.
    pub fn new(env: &'env Run, input: &str, smt_log_dir: &Option<String>) -> Res<Self> {
        use std::{fs::OpenOptions, io::Read};

        let smt_log_dir = smt_log_dir.clone();
        let mut file = OpenOptions::new().read(true).open(input)?;

        let mut txt = String::new();
        file.read_to_string(&mut txt)?;

        let sys = parse::trans(&txt)?;
        if env.verb >= 3 {
            println!("|===| Parsing {}:", env.styles.green.paint("successful"));
            for line in sys.to_ml_string().lines() {
                println!("| {}", line)
            }
            println!("|===|");
            println!()
        }

        Ok(Self {
            env,
            sys,
            smt_log_dir,
        })
    }

    /// Attemps to prove the candidates on a system.
    pub fn run(&self) -> Res<(BaseRes, StepRes)> {
        let base_res = self.base_check()?;
        let step_res = self.step_check()?;

        println!("|===| {} attempt result", self.bold.paint("Induction"));

        if base_res.has_falsifications() {
            println!(
                "| - the following candidate(s) are {} in the initial state(s)",
                self.red.paint("falsifiable")
            );
            for (candidate, _) in base_res.cexs.iter() {
                println!("|   `{}`", self.red.paint(*candidate))
            }
        } else {
            println!(
                "| - all candidates {} in the initial state(s)",
                self.green.paint("hold")
            );
        }

        println!("|");

        if step_res.has_falsifications() {
            println!(
                "| - the following candidate(s) are {} (not preserved by the transition relation)",
                self.red.paint("not inductive")
            );
            for (po, _) in step_res.cexs.iter() {
                println!("|   `{}`", self.red.paint(*po))
            }
        } else {
            println!(
                "| - all candidates are {} (preserved by the transition relation)",
                self.green.paint("inductive")
            );
        }

        println!("|");

        if !base_res.has_falsifications() && !step_res.has_falsifications() {
            println!(
                "| - system is {}, all reachable states verify the candidate(s)",
                self.green.paint("safe")
            )
        } else if base_res.has_falsifications() {
            println!(
                "| - system is {}, some candidate(s) are falsified in the initial state(s)",
                self.red.paint("unsafe")
            );
            if self.env.verb == 0 {
                println!(
                    "|   (run again without `{}` to see counterexamples)",
                    self.bold.paint("-v")
                )
            }
        } else if step_res.has_falsifications() {
            println!(
                "| - system {}, some candidate(s) are {}",
                self.red.paint("might be unsafe"),
                self.red.paint("not inductive"),
            );
            if self.env.verb == 0 {
                println!(
                    "|   (run again without `{}` to see counterexamples)",
                    self.bold.paint("-v")
                )
            }
        }

        if (base_res.has_falsifications() || step_res.has_falsifications())
            && base_res.okay.iter().any(|b_ok_candidate| {
                step_res
                    .okay
                    .iter()
                    .any(|s_ok_candidate| b_ok_candidate == s_ok_candidate)
            })
        {
            println!("|");
            println!(
                "| - the following candidate(s) {} in the initial state(s) and are {}",
                self.green.paint("hold"),
                self.green.paint("inductive")
            );
            println!(
                "|   and thus {} in all reachable states of the system:",
                self.green.paint("hold")
            );

            for candidate in base_res.okay.intersection(&step_res.okay) {
                println!("|   `{}`", self.green.paint(*candidate))
            }
        }

        println!("|===|");

        Ok((base_res, step_res))
    }

    /// Runs BMC.
    pub fn bmc(&self, max: Option<usize>, base: &BaseRes, step: Option<&StepRes>) -> Res<()> {
        let bmc_res = if let Some(step) = step {
            base.merge_base_with_step(step)
                .chain_err(|| "during base/step result merge for BMC")?
        } else {
            base.as_inner().clone().into()
        };
        if bmc_res.all_falsified() {
            return Ok(());
        }

        println!(
            "running {}, looking for falsifications for {} candidate(s)...",
            self.bold.paint("BMC"),
            bmc_res.okay.len()
        );

        let conf = SmtConf::z3(&self.env.z3_cmd);
        let tee = self.smt_log_dir.as_ref().map(std::path::PathBuf::from);
        let mut bmc = check::Bmc::new(&self.sys, conf, tee, bmc_res)?;
        let mut falsified = Set::new();

        while !bmc.is_done() && max.map(|max| max >= bmc.next_check_step()).unwrap_or(true) {
            let depth_str = bmc.next_check_step().to_string();
            if self.env.verb > 0 {
                println!(
                    "checking for falsifications at depth {}",
                    self.env.styles.under.paint(&depth_str)
                );
            }

            let new_falsifications = bmc.next_check().chain_err(|| {
                format!(
                    "while checking for falsifications at depth {} in BMC",
                    self.env.styles.under.paint(&depth_str)
                )
            })?;

            if new_falsifications {
                for (candidate, cex) in bmc.res().cexs.iter() {
                    let is_new = falsified.insert(candidate.to_string());
                    if is_new {
                        println!(
                            "found a {} at depth {}:",
                            self.red.paint("falsification"),
                            self.env.styles.bold.paint(&depth_str)
                        );
                        self.present_cex(&self.sys, candidate, cex, true)?
                    }
                }
            }
        }

        let bmc_res = bmc.destroy()?;

        if self.env.verb > 0 || !bmc_res.cexs.is_empty() {
            println!()
        }

        println!("|===| {} result", self.bold.paint("Bmc"));
        if !bmc_res.okay.is_empty() {
            println!(
                "| - could {} find falsifications for the following candidate(s)",
                self.bold.paint("not")
            );
            for candidate in &bmc_res.okay {
                println!("|   `{}`", self.bold.paint(candidate as &str))
            }
        }
        if !bmc_res.okay.is_empty() && !bmc_res.cexs.is_empty() {
            println!("|")
        }
        if !bmc_res.cexs.is_empty() {
            println!(
                "| - found a {} for the following candidate(s)",
                self.red.paint("falsification")
            );
            for candidate in bmc_res.cexs.keys() {
                println!("|   `{}`", self.red.paint(*candidate))
            }
        }
        println!("|");
        if !base.cexs.is_empty() || !bmc_res.cexs.is_empty() {
            println!("| - system is {}", self.red.paint("unsafe"))
        } else {
            println!("| - system {}", self.red.paint("might be unsafe"),);
            println!(
                "|   no falsification in {} was found for some candidate(s)",
                self.bold.paint(format!(
                    "{} step(s) or less",
                    max.expect(
                        "[fatal] cannot have BMC with no max end with unfalsified candidates"
                    ),
                )),
            );
        }
        println!("|===|");

        Ok(())
    }

    /// Performs the base check.
    pub fn base_check(&self) -> Res<BaseRes> {
        if self.env.verb > 0 {
            println!("checking {} case...", self.under.paint("base"))
        }
        let conf = SmtConf::z3(&self.env.z3_cmd);
        let tee = self.smt_log_dir.as_ref().map(std::path::PathBuf::from);
        let mut base_checker =
            check::Base::new(&self.sys, conf, tee).chain_err(|| "during base checker creation")?;
        let res = base_checker.check().chain_err(|| "during base check")?;
        if self.env.verb > 0 {
            if !res.has_falsifications() {
                println!(
                    "{}: all candidate(s) {} in the {} state",
                    self.green.paint("success"),
                    self.green.paint("hold"),
                    self.under.paint("base"),
                )
            } else {
                println!(
                    "{}: the following candidate(s) {} in the {} state(s):",
                    self.red.paint("failed"),
                    self.red.paint("do not hold"),
                    self.under.paint("initial")
                );
                self.present_base_cexs(&self.sys, &res)?
            }
            println!()
        }
        Ok(res)
    }

    /// Performs the step check.
    pub fn step_check(&self) -> Res<StepRes> {
        if self.env.verb > 0 {
            println!("checking {} case...", self.under.paint("step"))
        }
        let conf = SmtConf::z3(&self.env.z3_cmd);
        let tee = self.smt_log_dir.as_ref().map(std::path::PathBuf::from);
        let mut step_checker =
            check::Step::new(&self.sys, conf, tee).chain_err(|| "during step checker creation")?;
        let res = step_checker.check().chain_err(|| "during step check")?;
        if self.env.verb > 0 {
            if !res.has_falsifications() {
                println!(
                    "{}: all candidate(s) are {}",
                    self.green.paint("success"),
                    self.green.paint("inductive")
                )
            } else {
                println!(
                    "{}: the following candidate(s) are {}:",
                    self.red.paint("failed"),
                    self.red.paint("not inductive"),
                );
                self.present_step_cexs(&self.sys, &res)?
            }
            println!()
        }
        Ok(res)
    }

    pub fn present_base_cexs(&self, sys: &trans::Sys, res: &BaseRes) -> Res<()> {
        self.present_cexs(sys, res, true)
    }
    pub fn present_step_cexs(&self, sys: &trans::Sys, res: &StepRes) -> Res<()> {
        self.present_cexs(sys, res, false)
    }
    pub fn present_cexs<'sys, R: Deref<Target = CheckRes<'sys>>>(
        &self,
        sys: &trans::Sys,
        res: &R,
        is_base: bool,
    ) -> Res<()> {
        for (candidate, cex) in res.cexs.iter() {
            self.present_cex(sys, *candidate, cex, is_base)?
        }
        Ok(())
    }
    pub fn present_cex(
        &self,
        sys: &trans::Sys,
        candidate: &str,
        cex: &check::cexs::Cex,
        is_base: bool,
    ) -> Res<()> {
        let max_id_len = sys.decls().max_id_len();
        let def = sys.po_s().get(candidate).ok_or_else(|| {
            format!(
                "failed to retrieve definition for candidate `{}`",
                candidate,
            )
        })?;
        println!(
            "- `{}` = {}",
            self.red.paint(candidate),
            self.bold.paint(format!("{}", def))
        );
        for (step, values) in &cex.trace {
            let step_str = if is_base {
                format!("{}", self.under.paint(step.to_string()))
            } else {
                let mut step_str = format!("{}", self.under.paint("k"));
                if *step > 0 {
                    step_str = format!("{}{}", step_str, self.under.paint(format!(" + {}", step)))
                }
                step_str
            };
            println!("  |=| Step {}", step_str);
            for (var, cst) in values {
                let var_str = format!("{: >1$}", var.id(), max_id_len);
                println!("  | {} = {}", self.bold.paint(var_str), cst)
            }
        }
        if !cex.unexpected.is_empty() {
            println!("  |=| Z3 produced the following unexpected values");
            for (desc, val) in &cex.unexpected {
                println!("  | {} = {}", self.red.paint(desc.to_string()), val);
            }
        }
        println!("  |=|");
        Ok(())
    }
}

/// Stores the output styles.
pub struct Styles {
    /// Bold style.
    pub bold: Style,
    /// Underlined style.
    pub under: Style,
    /// Red style.
    pub red: Style,
    /// Green style.
    pub green: Style,
    /// Gray style.
    pub gray: Style,

    /// Italic.
    pub ita: Style,
    /// Code-style.
    pub code: Style,
}
impl mikino_api::prelude::Style for Styles {
    type Styled = String;
    fn bold(&self, s: &str) -> Self::Styled {
        self.bold.paint(s).to_string()
    }
    fn red(&self, s: &str) -> Self::Styled {
        self.red.paint(s).to_string()
    }
    fn green(&self, s: &str) -> Self::Styled {
        self.green.paint(s).to_string()
    }
    fn under(&self, s: &str) -> Self::Styled {
        self.under.paint(s).to_string()
    }
    fn gray(&self, s: &str) -> Self::Styled {
        self.gray.paint(s).to_string()
    }
    fn ita(&self, s: &str) -> Self::Styled {
        self.ita.paint(s).to_string()
    }
    fn code(&self, s: &str) -> Self::Styled {
        self.code.paint(s).to_string()
    }
}
impl Styles {
    /// Constructor, with colors activated.
    pub fn new_colored() -> Self {
        Self {
            bold: Style::new().bold(),
            under: Style::new().underline(),
            red: Colour::Red.normal(),
            green: Colour::Green.normal(),
            gray: Colour::Fixed(8).normal(),
            ita: Style::new().italic(),
            code: Colour::Yellow.normal(),
        }
    }

    /// Constructor, with color deactivated.
    pub fn new_no_color() -> Self {
        Self {
            bold: Style::new(),
            under: Style::new(),
            red: Style::new(),
            green: Style::new(),
            gray: Style::new(),
            ita: Style::new(),
            code: Style::new(),
        }
    }

    /// Constructor.
    #[cfg(any(feature = "force-color", not(windows)))]
    pub fn new(color: bool) -> Self {
        if color && atty::is(atty::Stream::Stdout) {
            Self::new_colored()
        } else {
            Self::new_no_color()
        }
    }

    /// Constructor.
    ///
    /// This Windows version always produces colorless style.
    #[cfg(not(any(feature = "force-color", not(windows))))]
    pub fn new(_: bool) -> Self {
        Self {
            bold: Style::new(),
            under: Style::new(),
            red: Style::new(),
            green: Style::new(),
        }
    }
}

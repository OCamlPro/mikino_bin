//! A simple demo system.
//! 
//! Systems are declared in four ordered parts:
//! 
//! - the variables of the system;
//! - the initial predicate;
//! - the transition predicate;
//! - the Proof Obligations (POs) to prove on the system.
//! 
//! Each part starts with a keyword followed by a colon `:`. The opening keywords are `vars`,
//! `init`, `trans` and `po_s` respectively.
//! 
//! # Init
//! 
//! The initial predicate is a constraint over the variables of the system. Any assignment of the
//! variables that makes this initial predicate true is a legal initial state.
//! 
//! Say your system is just a counter with a single variable `cnt: int`. If you want to start with
//! `cnt = 0`, then your initial predicate would be `(= cnt 0)`. If you want to start with any
//! positive value, then it would be `(>= cnt 0)` or `(≥ cnt 0)`. If you're fine with starting with
//! any value at all, then your initial predicate can just be `true`.
//! 
//! # Trans
//! 
//! The transition predicate describes whether a state can be the successor of another state, or the
//! "next" state. That is, the transition predicate is a constraint that mentions variables of
//! the "current" state and variables of the "next" state. If `v` is a variable of your system, then
//! the "current" value of `v` is written `(pre v)`. The "next" value is just written `v`.
//! 
//! Going back to the simple counter system example above, you can express that the "next" value of
//! `cnt` is the current value plus on with `(= cnt (+ (pre cnt) 1))`
//! 
//! # POs
//! 
//! A Proof Objective (PO) is a *name* and a definition, a predicate over the variables of the
//! system. The name is given as a double-quoted string, for instance `"my proof objective"`. The
//! predicate is a constraint over the variables (it cannot use the `pre` operator, it can only
//! refer to one state).
//! 
//! In the counter system from above, maybe we want to prove that `cnt` is always positive. We can
//! do so by having a PO called, for instance, `"cnt is positive"` defined as `(>= cnt 0)` or `(≥
//! cnt 0)`.
//! 
//! # This Example
//! 
//! This system is a stopwatch. It has a (time) counter `cnt`, which would be the time a real
//! stopwatch would actually display. It has two boolean variables `stop` and `reset` which would be
//! buttons on an actual stopwatch. Variable `reset` forces `cnt` to be `0` whenever it is true,
//! while `stop` freezes the value of `cnt` as long as it remains true. Note that `reset` has
//! priority over `stop`: if both are true then `cnt` will be forced to `0`.

/// Variables.
vars: (
    /// Stop and reset (inputs).
    stop, reset: bool
    /// Time counter (output).
    cnt: int
)

/// Initial predicate.
init: (and
    // `cnt` can be anything as long as it is positive.
    (≥ cnt 0)
    // if `reset`, then `cnt` has to be `0`.
    (⇒ reset (= cnt 0))
)

/// Transition predicate.
/// 
/// - `reset` has priority over `stop`;
/// - the `ite` stands for "if-then-else" and takes a condition, a `then` expression and an `else`
///   expression. These last two expressions must have the same type. In the two `ite`s below, that
///   type is always `bool`.
trans: (ite
    /// condition
    reset
    /// then
    (= cnt 0)
    /// else
    (ite
        /// condition
        stop
        /// then
        (= cnt (pre cnt))
        /// else
        (= cnt (+ (pre cnt) 1))
    )
)

/// Proof obligations.
po_s: (
    "cnt is positive": (≥ cnt 0)
    "cnt is not -7": (not (= cnt (- 7)))
    "if reset then cnt is 0": (⇒ reset (= cnt 0))
)

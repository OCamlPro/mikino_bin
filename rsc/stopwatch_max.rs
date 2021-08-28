// The constant `128` is used to represent the upper-bound of the `uint128` legal range.

vars: (
    play_pause, reset, running, paused, saturated: bool
    count, max: int
)

init: (⋀
    paused
    (¬ running)
    (= count 0)
    (≤ 1 max)
    (≤ max 128)
    (= saturated (= count max))
)

trans: (⋀
    (= max (pre max))
    (ite
        (⋀ (¬ (pre play_pause)) play_pause)
        (⋀
            (⇒ (pre running) (⋀ paused (¬ running)))
            (⇒ (pre paused) (⋀ running (¬ paused)))
        )
        (⋀
            (= running (pre running))
            (= paused (pre paused))
        )
    )
    (ite
        reset
        (= count 0)
        (ite (⋀ running (¬ (= (pre count) max)))
            (= count (+ (pre count) 1))
            (= count (pre count))
        )
    )
    (= saturated (= count max))
)

po_s: (
    "count in range": (⋀ (≤ 0 count) (≤ count 128))
    "max in range": (⋀ (≤ 0 max) (≤ max 128))
    "count real range": (⋀ (≤ 0 count) (≤ count max))
    "count positive": (≥ count 0)
    "reset semantics": (⇒ reset (= count 0))
    "modes are exclusive": (⋁ (¬ running) (¬ paused))
    "one mode active": (⋁ running paused)
)

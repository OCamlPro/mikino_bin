vars: (
    play_pause, reset, running, paused: bool
    count: int
)

init: (⋀
    paused
    (¬ running)
    (= count 0)
)

trans: (⋀
    (⇒
        (⋀ (¬ (pre play_pause)) play_pause)
        (⋀
            (⇒ (pre running) (⋀ paused (¬ running)))
            (⇒ (pre paused) (⋀ running (¬ paused)))
        )
    )
    (ite
        reset
        (= count 0)
        (⋀
            (⇒ running (= count (+ (pre count) 1)))
            (⇒ paused (= count (pre count)))
        )
    )
)

po_s: (
    "count in range": (⋀ (≤ 0 count) (≤ count 128))
    "count positive": (≥ count 0)
    "reset semantics": (⇒ reset (= count 0))
    "modes are exclusive": (⋁ (¬ running) (¬ paused))
    "one mode active": (⋁ running paused)
)

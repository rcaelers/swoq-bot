# SWOQ Bot

This is a bot for the [Sioux Weekend of Quest](https://github.com/drZymo/swoq) based on [Rust example](https://github.com/drZymo/swoq-bots).

Rust starter code taken from <https://github.com/drZymo/swoq-bots>.  
Images taken from <https://github.com/drZymo/swoq>.  
Copyright (c) 2025 Ralph Schiedon.

## TODO

- [ ] UI improvements
  - [x] Show pathfinder routes
  - [ ] Game should not start before visualizer is ready
  - [x] Show logging pane
- [ ] Refactoring vibe coded parts
  - [x] integrate_surroundings
  - [ ] generalize items handling (health, keys, swords)
  - [ ] ...
- [ ] Level 12: 2 Player mode
- [ ] AI strategies
- [ ] Strategy improvements / bug fixes
  - [x] incorrect enemies present in HuntEnemyWithSwordStrategy
  - [x] Hang when trying to go through door
  - [ ] flee enemy sometimes fails
  - [ ] Don't attack enemy if not needed
  - [ ] Enemy in range check should use pathfinder
  - [ ] Unexplored frontier: add unknown->known transitions
  - [x] If no further goals, pick random explored location in search for enemies

## License

MIT

- landing pads: https://releases.llvm.org/7.0.0/docs/ExceptionHandling.html https://releases.llvm.org/7.0.0/docs/LangRef.html#i-landingpad
- unwinding: https://doc.rust-lang.org/nomicon/unwinding.html
- promoted: constants extracted from a function and lifted to static scope - https://rust-lang.github.io/rustc-guide/appendix/glossary.html?highlight=static#appendix-c-glossary
- APALACHE TLA+ Model Checking https://blog.acolyer.org/2019/11/29/tla-model-checking-made-symbolic/


- deadlocks can be masked by unwinds
  - especially by arc around mutex
  - need unwind detection e.g:
    - remember unwind paths
    - check finishing transition to be a return (etc.)


# Outline

## Abstract
## Introduction
- motivation (maybe after rust)
- what is rust
  - what are the important features
- what is verification
  - difference to testing
  - different approaches (BDDs etc.)
    - use cases (parallel is good for petri nets)
- what are deadlocks
  - synchronisation
  - mutex/semaphore
  - threads
  - dining philosophers
- how can they be introduced in rust
  - rust and deadlocks -> considered safe code

## Approach
- concrete approach of verification
  - typical: language -> formalism
  - petri nets
  - tools -> LoLa

## Related Work
  - other verification implementations
    - (verification by language?)
      - functional programming invariants?
      - prolog invariants?
      - languages with verification methods in its design?
    - c verification
      - valgrind?
    - rust verification
  - petri net verification
    - bpel

## Translation
  - a rustc introduction
  - petri net formalism and format (high level? edge descriptions)
  - mapping of rust features in petri nets -> where can we use rust features to improve the translation
  - at what layer the translation takes place and why (@mir)
  - explain mir 
    - control-flow graphs
    - how to traverse
    - single elements
      - model of mutex/semaphore
    - a possible fitting petri net
    - how to deal with function calls
  - which parts can or must be excluded from translation
    - pre main
    - panics
    - missing mir parts
      - external libraries
      - intrinsics and platform specific behavior
    - std implementations
      - part between std::mutex and pthread mutex
  - joining the translations
    - joining basic elements
    - joining basic blocks
    - joining function calls
    - recursion limits
  - caching translations
  - lola integration (format of a petri net)
    - PNML

## Verification run with results?
  - expected deadlocks (expected program termination)
  - fitting formulas
    - exkurs in temporal logic
  - structure of results
  - examples
    - minimal deadlock
    - fixed minimal deadlock
    - deadlock with multiple threads
      - dining philosophers?
    - data dependent deadlock
    - maybe a real world example
      - some distributed system?
        - mqtt?
        - multiagent system?

## Conclusion

## Future Work
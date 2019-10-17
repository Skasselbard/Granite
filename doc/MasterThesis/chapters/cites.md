# Matsakis:2014:RL:2692956.2663188
- Furthermore, Rust’s type system goes beyond that of the vast majority of safe languages in that it statically rules out data races (which are a of undefined behavior for concurrent programs in many languages like C++ or Rust), as well as common programming pitfalls like iterator invalidation
- In its simplest form, the idea of ownership is that, although multiple aliases to a resource may exist simultaneously, performing certain actions on the resource (such as reading and writing a memory location) should require a "right" or "capability" that is uniquely "owned" by one alias at  any point during the execution of the program.
- [rust ownership] is enforceable automatically and eliminates a wide range of common low-level programming errors, such as "use after free", data races, and iterator invalidation. 
- Race conditions can only arise from an unrestricted combination
of aliasing and mutation on the same location. In fact, it turns out that ruling out mutation of
aliased data also prevents other errors commonplace in low-level pointer-manipulating programs,
like use-after-free or double-free. 
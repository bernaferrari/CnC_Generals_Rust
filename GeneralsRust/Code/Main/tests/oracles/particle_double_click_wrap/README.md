# Particle Cannon unsigned click-gap C++ witness

Run from this directory with `python3 verify_cpp_witness.py`. It verifies the full SHA-256 of the original `ParticleUplinkCannonUpdate.cpp`, extracts the actual double-click `if` expression, compiles that expression with a minimal `UnsignedInt`/data shim, and executes three finite and three wrap-boundary cases. The harness uses the retail default delay (`500 ms * 30 logic frames / 1000 = 15`). It tests the expression only; it does not execute the original engine update loop.

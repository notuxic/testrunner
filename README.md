Testrunner
==========

*Testrunner* is a input/output-based testing tool, written for courses teaching *C* and *C++*.

It aims to provide a comprehensive tool for checking programs for correctness in the context of
teaching *C* and *C++* (correct program behaviour on *I/O*, compiler warnings, memory leaks, ...),
while also generating a report that can be understood by less experienced users.


Features
--------

The *testrunner* is designed to be both, distributed along with public testcases to users for local testing, as well
as used for testing on a central testsystem with public _and_ secret/private testcases.

It is configured using a *TOML* configuration file, along with additional files containing the input
and reference output for the various testcases.

General features:

 - Generate a HTML report, showing results in a user-friendly format, including ...
    - a two-way line-diff for output and text files, including inline-diff-hints and whitespace-hints,
    - a two-way binary-diff in a *hexdump*-like format for binary files,
    - the input sent to the program,
    - the commandline used for running the program,
    - the programs exit-code,
    - other information, when using the respective feature (see bullet points below).
 - Generate a JSON report, for processing by other programs.
 - *IoTest*: A basic input/output test. Sends configured input to the program, and compares the output with a reference output.
 - *OrdIoTest*: A pseudo-interactive input/output test. Compared to *IoTest*, it simulates interactive use of the program.
 - Check an additional file, generated/modified by the tested program. Supports text- and binary-diff modes.
 - Detect and display compiler warnings, by compiling the code using a *Makefile* (supports *GCC* and *Clang*).
 - Detect and display memory usage errors and memory leaks, using *valgrind*.
 - Time limits for testcases.
 - Running multiple testcases in parallel.


Testsystem features:

 - *protected-mode*: Show only limited information for testcases marked as *protected* (secret/private testcases).
 - Run tested program under another user.


Planned features:

 - Unit tests


Building
--------

*Testrunner* officially supports *Linux*, and may also work on other *unix-like* operating-systems.

See [BUILDING.md](./BUILDING.md) for various considerations regarding building the project.


Documentation
-------------

See [testrunner.adoc](./testrunner.adoc) for command-line usage and a general overview.

See [testrunner-config.adoc](./testrunner-config.adoc) for configuring testcases.


Authors
-------

In order of initial involvement in the project:

 - Thomas Brunner (original author)
 - Mathias Kahr
 - Florian Hager
 - Kilian Payer
 - Julia Herbsthofer (current maintainer)


History
-------

This tool is developed for, and by the team of, the first-year university courses 
*Einführung in die strukturierte Programmierung* (*Introduction to Structured Programming*) and
*Objektorientierte Programmierung 1* (*Object-oriented Programming 1*) at *Graz University of Technology*.

Early on, the project got moved into a mono-repository; some time later, it got moved out into
its own dedicated repository again. For this reason, much of the early development history
is not available in the commit history of this repository.


License
-------

[Apache-2.0](./LICENSE)


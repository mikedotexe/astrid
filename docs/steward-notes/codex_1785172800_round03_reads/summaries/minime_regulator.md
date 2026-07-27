# Minime Regulator

Astrid accurately identified the current regulator file as a small include shell
that retains PD-mode API types while describing the engine as PI-only. The
report's "ghost architecture" hypothesis is not supported: the source marks
those declarations as dead-code/API-completeness and their presence cannot
affect runtime state by itself.

The included viscosity and pressure-source modules already provide extensive
bounded mechanical review. They repeatedly state that the values are inert,
read-only, and not regulator authority. Astrid's viscous-persistence report
therefore remains primary qualitative evidence beside those descriptors, not a
consequence established by them.

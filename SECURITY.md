# Security Policy / Política de seguridad

## English
Kubit is pre-release software. Please report vulnerabilities **privately**, not in public issues:

- Use GitHub's private advisory form: *Security* tab → *Report a vulnerability*, or
- email design@albertobalbuena.com.

Include the affected version or commit, a description, and a reproducer (for file-format issues, the
input file). We aim to acknowledge reports within 7 days and to coordinate disclosure with you.

Areas of particular interest: the `.kubit` and STEP/STL parsers (hostile input must never panic or
exhaust memory), the MCP server, and supply-chain issues in dependencies (`cargo deny` runs in CI).

Kubit's DFM and simulation results are design assistance, not engineering certification.

## Español
Kubit es software pre-lanzamiento. Reporta las vulnerabilidades en **privado**, no en issues públicos:
usa el formulario de avisos privados de GitHub (pestaña *Security* → *Report a vulnerability*) o
escribe a design@albertobalbuena.com, con versión o commit afectado, descripción y un caso para
reproducirlo. Intentamos acusar recibo en 7 días y coordinar la divulgación contigo.

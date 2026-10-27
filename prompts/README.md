# Prompts para Claude Code

## Orden de uso
1. `00-arranque.md`: Fase 0 (spike de kernels → esqueleto). Úsalo primero.
2. Para cada fase siguiente, copia la plantilla de abajo y llénala con los puntos de esa fase en `docs/ROADMAP.md`.

## Buenas prácticas
- **Una fase por sesión** (o por worktree). Al cerrar una fase, pide a Claude Code que actualice `docs/STATUS.md` y `docs/ROADMAP.md`, y empieza la siguiente con contexto limpio.
- **Plan Mode siempre al inicio.** Revisa el plan antes de aprobarlo; es el momento más barato para corregir el rumbo.
- **Puntos de pausa explícitos** en tareas con decisiones caras ("DETENTE y espera mi aprobación").
- **Pide evidencia, no promesas**: salida real de tests, capturas, ADRs con datos.
- **Revisa los diffs** de CLAUDE.md, ARCHITECTURE.md y los ADR con especial cuidado. Son la memoria del proyecto.
- Usa `/compact` cuando el contexto crezca, después de que Claude Code haya escrito `docs/STATUS.md`.
- Para trabajo paralelo (por ejemplo, catálogo y UI a la vez), usa **worktrees** de git, uno por tarea.

## Plantilla de prompt por fase

```
<rol>
Eres el ingeniero principal de Kubit. [Agrega la especialidad de la fase: p. ej. "experto en solvers de restricciones geométricas" / "experto en fabricación de mobiliario y nesting"].
</rol>

<contexto>
Lee CLAUDE.md, docs/STATUS.md, docs/ROADMAP.md (Fase N) y [docs específicos de la fase: p. ej. docs/MANUFACTURING-MODEL.md y docs/research/materiales-dfm.md §C].
Estado actual: [1-3 líneas o "ver STATUS.md"].
</contexto>

<objetivo>
Completar la Fase N: [copiar las casillas de la fase].
[Si hay una decisión cara: "Etapa A: … → DETENTE y espera mi aprobación. Etapa B: …"]
</objetivo>

<restricciones>
- Reglas de CLAUDE.md.
- Fuera de alcance: [lo que NO se debe tocar en esta fase].
- [Restricciones del dominio: p. ej. "toda regla DFM sin fuente va con confidence = typical-unverified y severidad máxima warn"].
</restricciones>

<criterios_de_aceptacion>
- [Copiar el criterio de salida de la fase del ROADMAP, en términos verificables.]
- `cargo xtask ci` en verde.
</criterios_de_aceptacion>

<formato_de_reporte>
Qué se hizo · Evidencia · Decisiones (ADR) · Riesgos · Siguiente paso.
</formato_de_reporte>
```

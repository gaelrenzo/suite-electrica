# Catálogos y tablas normativas

Aquí irán los CSV de conductores, postes, transformadores y tablas del CNE
Suministro 2011 / normas DGE-MINEM (fases LineCraft y GridCraft).

Reglas:

1. Cada CSV lleva en la primera línea un comentario `# fuente: <norma, tabla, página>`.
2. Si no hay fuente verificable, el archivo se nombra `*_PENDIENTE_REVISION.csv`
   y el código debe emitir un aviso al usarlo.
3. No se copian tablas de software comercial.

Estado: `conductores_PENDIENTE_REVISION.csv` (AAAC 35 del proyecto Capachica).
Copia usada por `linecraft nuevo`: `suite_electrica/plantillas/conductores.csv`
(mantener ambas iguales).

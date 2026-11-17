# Suite Eléctrica frente a ESOLUTIONS: comparación para mejorar

Comparación función por función con REDLIN 2023, REDCAD 2023, PRESIO 2023 y TOPOMAGIC 2020,
tomada de sus manuales de usuario (índices y capítulos citados) y de las validaciones numéricas
hechas con sus ejemplos (detalle en `PLAN_RUST.md`). Estado al 17/11/2026, versión `v0.2.4`.

**Leyenda:** ✅ igual y validado con el programa · ✔ la suite lo tiene (sin comparar o sin
equivalente) · ◐ parcial · ✗ falta.

## 1. Resumen

| Programa | Cálculo | Funciones | Lo que más falta |
|---|---|---|---|
| REDLIN | ✅ mismas cifras (catenaria, flechado, creep, cargas, postes, biposte H, terminal a media línea) | ◐ | Alertas de arrancamiento, cruceta y vano peso; plantilla de planos con bloques; terminales de derivación (TS) |
| REDCAD | ✅ caída de tensión BT, metrado, materiales | ◐ | Autodistribución BT desde lotes, capas, catastro como geodata, plantillas DXF |
| PRESIO | ✅ APU, fórmula polinómica, transporte | ◐ | Base de precios por región y desagregado de gastos generales |
| TOPOMAGIC | sin comparar (validado contra la versión Python) | ◐ | Numeración de viviendas, ortofotos, vías, unir/separar rutas, códigos de campo |

## 2. REDLIN (líneas primarias)

| Función (manual) | REDLIN | Suite | Nota |
|---|---|---|---|
| Importar topografía y rutas (§4.2) | sí | ✔ | CSV, DXF, KML, GPX, Excel RLT, SRTM |
| Exportar topografía a Excel (§4.3) | sí | ✔ | perfil.xlsx |
| Criterios de diseño (§4.4) | panel | ✔ | parámetros del proyecto con fuente |
| Diseño en vista perfil y planta (§4.6–4.7) | sí | ✔ | arrastrar, agregar y eliminar en ambas vistas |
| Cálculo mecánico del conductor (§9.2) | sí | ✅ | catenaria/parábola, cambio de estado, creep |
| Tiro vs. temperatura / tabla de flechado | sí | ✅ | |
| Cargas y momentos de postes, FS | sí | ✅ | con retenida en ángulo, anclaje, biposte H, terminal a media línea |
| Terminal de derivación (TS) | sí | ◐ | difiere: falta el ángulo de la derivación (ver PLAN, punto 3) |
| Alerta despeje al terreno, vista semáforo (fig. 27) | sí | ✔ | alerta DMS y colores en el perfil |
| Alerta separación horizontal a medio vano (fig. 29) | sí | ✔ | alerta H |
| Alerta fuerzas longitudinales (fig. 31) | sí | ✔ | FS poste (L) |
| **Alerta fuerzas verticales positivas** (fig. 33) | sí | ✗ | arrancamiento: vano peso negativo en estructuras bajas |
| Alerta rotura de espiga (fig. 35) | sí | ✔ | carga máx. por espiga |
| **Alerta rotura de cruceta** (fig. 37) | sí | ✗ | requiere la carga de rotura de cada cruceta en el catálogo |
| **Alerta de vano peso** (fig. 39) | sí | ✗ | límite de vano peso por armado |
| Vibraciones eólicas y amortiguadores (§9.5, fig. 41) | CIGRE TB 273 | ✔ | alerta H/w y L·D/m; amortiguadores por tabla del fabricante |
| Esquemas de aislamiento y ensambles (fig. 25) | sí | ✗ | proyección de ensambles-aislador y oscilación de cadenas |
| Distancias de seguridad a cruces | sí | ✔ | tabla de cruces con fuente del CNE |
| Base de datos editable, export/import (§7) | sí | ◐ | catálogos editables en la app; sin importar/exportar la base completa |
| **Plantilla de planos con bloques (§8)** | sí | ✗ | la suite exporta DXF con cajetín, sin bloques del usuario |
| Reportes de planos y planillas (§10.2–10.3) | sí | ✔ | perfil DXF con lámina, planilla Excel, memoria PDF |
| Reportes de todas las derivaciones (§10.4) | sí | ◐ | una línea por proyecto |
| Teclas rápidas (§11.1) | sí | ◐ | Ctrl+N/O/S/Z, F6, Supr |

## 3. REDCAD (redes primarias y secundarias)

| Función (manual) | REDCAD | Suite | Nota |
|---|---|---|---|
| Cargar plano catastral DXF (§4.2) | sí | ✔ | fondo DXF en GridCraft; catastro de predios en TopoCraft |
| Catastro como geodata (lotes seleccionables) | sí | ✗ | base del autodiseño de REDCAD |
| Parámetros de la localidad (§4.4) | sí | ✔ | parámetros de la red con fuente |
| Diseño de media tensión (§4.5) | sí | ✔ | red primaria 22,9 kV (sin equivalente de caída MT en REDCAD) |
| Diseño de baja tensión (§4.6) | sí | ✔ | editor en planta |
| Caída de tensión, vista semáforo (§4.7) | sí | ✅ | 106 tramos, ≤ 0,06 % |
| Selección, mover, deshacer/rehacer (§5) | sí | ✔ | Mayús+clic, Ctrl+Z, Ctrl+Y |
| **Autodistribución de estructuras BT desde lotes (§6.1.3)** | sí | ✗ | vanos mín./prom./máx. y vano flojo |
| Autodistribución de suministros BT (§6.1.4) | sí | ◐ | asignación múltiple manual; sin regla automática por normativa |
| Autodiseño de suministros MT (§6.1.5) | sí | ✗ | armados MT por regla de la base de datos |
| Autodiseño de conductores | no | ✔ | exclusivo de la suite |
| Robot supervisor del diseño (fig. 28) | sí | ✔ | revisión automática (hoja «Revisión») |
| **Capas (§6.5)** | sí | ✗ | mostrar/ocultar MT, BT, AP, catastro |
| Metrado y materiales | sí | ✅ | 33/33 renglones; 18/22 materiales de cantidad fija |
| Plantilla DXF personalizada y bloques (§8) | sí | ✗ | |
| Reportes de varias redes (§9.4) | sí | ◐ | metrado sumado en PresuCraft; sin planillas combinadas |
| Importar base de datos a varias redes (§7.5) | sí | ✗ | |

## 4. PRESIO (presupuesto)

| Función | PRESIO | Suite | Nota |
|---|---|---|---|
| Presupuesto por unidades constructivas | sí | ✅ | |
| Análisis de precios unitarios | sí | ✅ | 102/102 partidas |
| Fórmula polinómica | sí | ✅ | 3/3 fórmulas |
| Transporte por origen (hoja «Flete T») | sí | ✅ | 164/172 renglones iguales, resto ≤ 0,005 S/ |
| Metrado desde REDLIN/REDCAD | sí | ✔ | desde LineCraft y GridCraft (varias redes) |
| **Base de precios por región** | sí | ✗ | la suite trae precios de EJEMPLO |
| Desagregado de gastos generales | sí | ✗ | la suite usa un porcentaje |
| Cronograma | — | ✗ | |

## 5. TOPOMAGIC (topografía)

| Función (manual) | TOPOMAGIC | Suite | Nota |
|---|---|---|---|
| Importar topografía (§4.1–4.3) | sí | ✔ | sin importación masiva de varios archivos |
| **Importar ortofotos (§4.4)** | sí | ✗ | fondo georreferenciado |
| Exportar topografía (§4.5) | sí | ◐ | plano DXF; sin exportar puntos a CSV/Excel |
| Selección de puntos y objetos (§5.1–5.3) | sí | ◐ | tabla de puntos; sin selección en el plano |
| Insertar puntos manualmente (§6.2) | sí | ◐ | en la tabla, no en el plano |
| Crear rutas y vértices (§6.3–6.4) | sí | ✗ | el eje se importa |
| **Vías (§6.5–6.7)** | sí | ✗ | |
| Medición (§6.8) | sí | ✗ | |
| Catastro (§7.1) | sí | ✔ | predios afectados por la faja |
| Curvas de nivel (§7.3) | sí | ✔ | |
| **Numeración y validación de viviendas (§7.4–7.5)** | sí | ✗ | entrada del autodiseño BT |
| Traslación, rotación, elevación (§7.7–7.9) | sí | ✔ | editor de puntos |
| Proyectar puntos sobre ruta (§7.11) | sí | ✗ | |
| **Unir, separar, empalmar rutas (§7.12–7.15)** | sí | ✗ | |
| Invertir ruta (§7.14) | sí | ✔ | conserva estructuras y cruces |
| Perfil desde geoservidor (§7.16) | sí | ◐ | SRTM desde archivo `.hgt`, sin descarga |
| Perfil desde curvas de nivel (§7.17) | sí | ✗ | |
| Ver en Planeta (§7.18) | sí | ✗ | exportar KML para Google Earth |
| Códigos de campo y CTU (§8–9) | sí | ✗ | levantamiento de lotes con prefijos |

## 6. Mejoras priorizadas

Criterio: primero lo que cambia resultados o la seguridad del diseño; después lo que más tiempo
ahorra en un proyecto como el LP Capachica–Llachón; al final la comodidad.

### Prioridad 1: verificaciones que faltan (seguridad del diseño)
1. **Arrancamiento (fuerzas verticales positivas)**: alerta cuando el vano peso de una estructura
   queda negativo o bajo el mínimo en alguna hipótesis (típico en hondonadas). El vano peso ya se
   calcula; falta la alerta y el umbral con su fuente.
2. **Rotura de cruceta**: columna de carga de rotura en el catálogo de crucetas o armados y
   verificación de la fuerza por fase, como la de espiga.
3. **Vano peso máximo por armado**: límite en el catálogo de armados y alerta.
4. **Terminales de derivación (TS)**: pedir el ángulo de la derivación y combinar las cargas
   (ya existe `derivaciones.rs`); validar con REDLIN mientras dure la demo.

### Prioridad 2: lo que más tiempo ahorra
5. **Autodistribución de estructuras BT desde lotes**: a partir de los predios del catastro (ya
   importados) y de vanos mínimo/promedio/máximo, ubicar los postes de la red secundaria. Base:
   numeración de viviendas (6).
6. **Numeración y validación de viviendas** sobre el catastro.
7. **Exportar KML** (planta de la línea y la red, «Ver en Planeta») y **exportar puntos** a CSV.
8. **Capas** en GridCraft y TopoCraft (mostrar/ocultar MT, BT, AP, catastro, fondo).
9. **Unir, separar y empalmar rutas**; proyectar puntos sobre la ruta.

### Prioridad 3: entrega y presentación
10. **Plantilla de planos con bloques** del usuario (DXF), además del cajetín actual.
11. **Esquemas de aislamiento**: ensambles por armado y oscilación de cadenas de suspensión.
12. **Base de precios por región** y desagregado de gastos generales en PresuCraft.
13. **Ortofotos** como fondo georreferenciado.
14. Planillas combinadas de varias redes y de todas las derivaciones.

### Ya no hace falta perseguir
- Autodiseño de conductores y caída de tensión en MT: REDCAD no los tiene; la suite los valida
  con cálculo a mano y con la fórmula de la DGE/MEM-DEP 501.
- Robot supervisor: cubierto por la revisión automática del diseño.

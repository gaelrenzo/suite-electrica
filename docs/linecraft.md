# LineCraft — fundamentos de cálculo

Unidades SI: m, N, Pa, °C. Los catálogos se leen en unidades de ficha técnica
(mm², mm, kg/m, kN, MPa) y se convierten al cargarlos. g = 9,80665 m/s².

## 1. Entradas y valores normativos

El proyecto (`proyecto.json`) contiene todos los valores normativos como
**entradas**: hipótesis (temperatura, presión de viento, hielo), tiro inicial
(EDS, % de rotura), tiro máximo admisible (% de rotura) y distancia mínima al
terreno. Cada uno debe citar su fuente en `fuentes`; si falta o dice
"PENDIENTE"/"EJEMPLO", aparece como aviso `PENDIENTE_REVISION` en consola,
planilla y memoria. El código no contiene valores del CNE.

Los valores de `examples/linecraft/` vienen del proyecto académico Capachica
(AAAC 35 mm², hipótesis I–V, EDS 18 %, tiro máx. 40 %, 6,5 m) y están marcados
como EJEMPLO/PENDIENTE_REVISION.

## 2. Cargas unitarias (`conductor.py`)

* Peso propio: `w_c = m·g`
* Hielo: `w_h = ρ_h·g·π·e·(d + e)` (ρ_h es una entrada de la hipótesis)
* Viento: `w_v = p·(d + 2e)`, con p (Pa) dado por la hipótesis
* Resultante: `w = √((w_c + w_h)² + w_v²)`

Verificado contra Capachica: I → 0,2008 kgf/m, II → 0,3434 kgf/m.

## 3. Ecuación de cambio de estado (`mecanica.py`)

Igualando la variación de longitud parabólica `a³w²/(24H²)` con la dilatación
térmica y la deformación elástica:

```
σ2²·[σ2 − σ1 + a²·w1²·E/(24·A²·σ1²) + α·E·(t2 − t1)] = a²·w2²·E/(24·A²)
```

σ = H/A (esfuerzo horizontal). Cúbica con una única raíz real positiva
(`numpy.roots`). Se aplica al **vano regulador** de cada tramo de tensado:
`a_r = √(Σa³ / Σa)`.

Fuente: Checa, L. M., *Líneas de transporte de energía*, 3.ª ed., Marcombo,
1988, cálculo mecánico de conductores (ecuación de cambio de condiciones y
vano de regulación).

Validación (`tests/test_linecraft_mecanica.py`): reproduce la tabla de
hipótesis de Capachica (vano 115,59 m) con error < 0,05 kgf; prueba
independiente del balance de longitudes.

## 4. Curva del conductor

Soportes A (x = 0) y B (x = a, desnivel h). Parámetro `C = H/w`.

* **Catenaria**: `y(x) = C·[cosh((x − x_m)/C) − cosh(x_m/C)]`,
  `x_m = a/2 − C·asinh(h / (2C·sinh(a/2C)))`.
  Flecha (máxima distancia vertical cuerda–conductor) en
  `x* = x_m + C·asinh(h/a)`. Tiro máximo `H·cosh(max(|x_m|, |a − x_m|)/C)`.
  Longitud `√(h² + (2C·sinh(a/2C))²)`.
* **Parábola**: `y(x) = h·x/a − w·x·(a − x)/(2H)`, `f = w·a²/(8H)`,
  `x_m = a/2 − H·h/(w·a)`.

Flecha vertical = flecha en el plano de la carga resultante × `w_vertical / w`.

## 5. Ubicación de estructuras (`estructuras.py`)

* Manual: CSV `progresiva_m[,tipo][,altura_amarre_m]`; debe incluir inicio, fin
  y todos los vértices del eje (si no, error).
* Automática (voraz): estructuras obligatorias en los vértices; desde cada
  estructura se toma el vano más largo ≤ vano máximo cuyo despeje mínimo al
  terreno, con la hipótesis de flecha máxima, sea ≥ distancia mínima. El tiro de
  esa hipótesis depende del vano regulador, por lo que se itera hasta que este
  converge (< 0,5 m).
* Tipos: FIN (extremos), ANCLAJE (separa tramos de tensado), ANGULO (vértice
  con deflexión), SUSPENSION.

Despeje: el conductor se muestrea cada ≤ 1 m y en todos los quiebres del perfil
(el terreno TIN es lineal entre ellos, así que el mínimo es exacto en el terreno).
La altura de amarre corresponde al conductor más bajo.

## 6. Cargas en estructuras (`calculo.py`)

Para n conductores, deflexión θ, tiros H atrás/adelante:

* Vano viento: `a_v = (a_atrás + a_adelante)/2`
* Vano peso (gravivano): distancia entre los puntos bajos de los vanos
  adyacentes, `(a_atrás − x_m,atrás) + x_m,adelante`. Si es negativo se avisa
  (arrancamiento).
* Transversal: `n·[(H_a + H_b)·sen(θ/2) + w_v·a_v·cos(θ/2)]`
* Longitudinal: `n·|H_adelante − H_atrás|·cos(θ/2)` (en FIN: `n·H`)
* Vertical: `n·w_vertical·a_peso`
* Momento a nivel del terreno: `F_h·h_amarre`
* Retenida (FIN, ANCLAJE o ángulo): `T = F_h / cos φ`, φ respecto de la horizontal.

No incluye viento sobre poste ni armado, ni selección de poste/retenida (requiere
catálogos con fuente en `data/`).

## 7. Alertas de diseño (`alertas.py`)

Mismo criterio que las alertas de REDLIN: exceso relativo sobre el valor
permisible `E = (P* − P) / P` (P* evaluado, P máximo con su factor de
seguridad). Solo se reporta si `E > 0`; por elemento se marca *crítica* la de
mayor E. Hoja **Alertas** de la planilla y sección de la memoria PDF.

| Código | Verificación | P* | P |
|---|---|---|---|
| DMS | Despeje al terreno (vano) | DMS exigida | despeje real (`E = DMS/d − 1`) |
| C | Tiro del conductor (tramo, cada hipótesis) | % rotura | tiro máximo % |
| H | Separación horizontal a medio vano | `D = k_U·U + k_f·√f` | menor `separacion_fases_m` de los armados |
| V | Vibraciones eólicas | tiro horizontal EDS % | `eds_max_pct` |
| T / L | Poste sin retenida (T si predomina la transversal) | FS exigido | FS real |
| R | Retenida | FS exigido | FS real |
| S↓ | Compresión: `V + T_ret·sen φ` | — | `carga_vertical_max_daN` del poste |
| S↑ | Vano peso negativo, armado con `aislador=RIGIDO` | — | (∞) |
| I↑ / I↓ | Ángulo de salida del conductor en aislador rígido | ángulo | `angulo_salida_max/min` |
| I→ | Fuerza por fase en la espiga | `F_h / n` | `tiro_max_espiga × aisladores_por_fase` |

El ángulo de salida es la pendiente del conductor (curva con carga vertical)
al salir del soporte hacia cada vano, positiva hacia arriba, en todas las
hipótesis.

Todos los límites valen 0 por defecto = **no se verifica** (no se asumen
valores normativos). Al activarlos se exige su fuente en `fuentes`
(`sep_k_tension`, `sep_k_flecha`, `angulo_salida_min/max`, `tiro_max_espiga`,
`eds_max_pct`). En `proyecto.json` van en un bloque opcional
`"alertas": {"eds_max_pct": ..., ...}` con los nombres de `ConfigAlertas`.

No implementado aún: crucetas, coordinación de flechas entre conductores,
número de amortiguadores.

## 8. Validación contra REDLIN 2023

Ejemplo *Sistema eléctrico rural* de REDLIN (11 derivaciones, 270 estructuras,
AAAC 35/50/70, norma DGE/MEM). Con el parámetro de catenaria `C`, el vano y el
desnivel de amarres que reporta REDLIN, la catenaria de LineCraft reproduce:

| Magnitud | Vanos | Error máx. |
|---|---|---|
| Flecha a medio vano | 256 | < 0,1 mm |
| Vano peso | 245 | 1 cm |
| Vano viento | 264 | 0 (ver nota) |
| Tiro H vs temperatura (15–35 °C, *Tabla de Flechado*) | 1280 puntos / 256 tramos | mediana 0,001 %; p95 0,43 % en vanos ≥ 20 m |
| C en *Máximo Esfuerzo*, estado final con creep (tramos de un vano) | 152 | mediana 0,48 % (lineal sin creep: 10 %) |
| Ídem, tramos de varios vanos | 104 | mediana 6,3 % (ver nota) |

* REDLIN reporta la **flecha a medio vano**; LineCraft reporta además la flecha
  máxima (paralela a la cuerda), que en vanos inclinados es algo mayor. Ambas van
  en la hoja *Vanos*.
* En el primer vano de una derivación REDLIN no suma el vano de enlace al vano
  viento (6 estructuras); LineCraft aún no modela derivaciones.
* Cambio de estado: en tramos de **un vano con desnivel** se usa la forma exacta
  (longitud de catenaria y longitud sin tensión); con la cúbica de vano a nivel el
  error llegaba a 39 % en vanos cortos muy inclinados. Los únicos desvíos > 3 % son
  vanos flojos < 20 m (EDS 2–7 %).
* **Creep (fluencia lenta)**: si el conductor tiene `curva_creep` (y opcional
  `curva_inicial`), coeficientes `a0;…;a4` de σ (kg/mm²) vs ε (%) del fabricante,
  las hipótesis distintas de la inicial se calculan con el alargamiento permanente
  del método gráfico de Alcoa, `Δε = [ε_c(σf) − σf/E] − [ε_i(σ1) − σ1/E]`, con el
  esfuerzo final σf iterado, aplicado como temperatura equivalente `Δε/α`
  (`condicion_final = "creep"`; `"inicial"` lo desactiva).
* En tramos de varios vanos REDLIN reporta en los estados finales un C distinto por
  vano (cada vano con su propio largo, como con aisladores rígidos); LineCraft usa el
  vano regulador del tramo. Calculando cada vano por separado el error baja a 2,7 %
  (pendiente de implementar como opción).
* REDLIN elige las hipótesis por rango de altitud de cada tramo; en LineCraft el
  proyecto define un solo juego de hipótesis.
* La longitud de catenaria coincide por construcción (de ella se despeja el
  desnivel de amarres, que depende de la altura de cada armado).

Script reproducible (requiere exportar la planilla desde REDLIN; no se
distribuyen sus datos): `examples/validacion/comparar_redlin.py PlanillaEstTotal.xls` y
`... TablaFlechado.XLS Suministros.XLS --flechado` y
`... PlanillaEstTotal.xls TablaFlechado.XLS Suministros.XLS Configuracion.XLS --final`.

## 9. Limitaciones conocidas

* Ubicación automática voraz: puede dejar vanos cortos antes de un vértice; no
  optimiza costo.
* Un único conductor de diseño (el más bajo) para despejes; no se verifican
  cruces (carreteras, otras líneas) ni distancias horizontales.
* Cambio de estado parabólico (válido para flechas pequeñas respecto del vano).

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

## 7. Limitaciones conocidas

* Ubicación automática voraz: puede dejar vanos cortos antes de un vértice; no
  optimiza costo.
* Un único conductor de diseño (el más bajo) para despejes; no se verifican
  cruces (carreteras, otras líneas) ni distancias horizontales.
* Cambio de estado parabólico (válido para flechas pequeñas respecto del vano).

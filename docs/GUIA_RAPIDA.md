# Guía rápida

## 1. Instalar

**Windows 10/11**

1. Instale Python 3.11 o superior desde <https://www.python.org/downloads/> y
   marque **"Add python.exe to PATH"**.
2. Descargue el repositorio (`git clone git@github.com:gaelrenzo/suite-electrica.git`
   o *Code → Download ZIP*).
3. Haga doble clic en **`instalar.bat`**. Crea `.venv`, instala todo (incluida la
   interfaz gráfica), ejecuta las pruebas, agrega `bin\` al PATH y crea el acceso
   directo **Suite Electrica** en el Escritorio y el menú Inicio.
4. Abra la app desde el acceso directo. Los comandos `topocraft`/`linecraft`
   funcionan en una **nueva** terminal.

**Linux / Termux-Ubuntu**

```bash
cd suite-electrica
python3 -m pip install -e ".[dev,gui]"   # en Debian/Ubuntu del sistema: añadir --break-system-packages
suite-electrica                          # app (requiere pantalla: escritorio, X11 o VNC)
topocraft --help
```

## 1b. La app (Suite Eléctrica)

Todo el trabajo se guarda en **un solo archivo `.sel`** (Archivo → Guardar, Ctrl+S):
puntos, eje, parámetros, conductor, hipótesis, fuentes, estructuras y catálogos.

**Flujo de trabajo**

1. **TopoCraft** → *Importar puntos* y *Importar eje* → **Procesar** (F5): plano con
   curvas, perfil, tabla y registro de validación. Exportar plano/perfil si se requiere.
2. **LineCraft** → revise las pestañas de datos:
   * *Datos*: parámetros de diseño (EDS, tiro máximo, distancia mínima, vanos, FS…).
   * *Conductor*: ficha del conductor (o *Cargar desde catálogo CSV*).
   * *Hipótesis*: tabla editable (+ Fila / − Fila).
   * *Fuentes*: cite la norma de cada valor; lo que diga PENDIENTE aparece como aviso.
   * *Catálogos*: postes, armados, retenidas y materiales (editar, importar/exportar CSV).
3. **Calcular** (F6). La primera vez ubica las estructuras automáticamente y asigna
   armados y postes.
4. **Perfil interactivo**:
   * **Doble clic** en zona libre: agrega estructura.
   * **Arrastrar** un poste: lo mueve (inicio, fin y vértices ■ están fijos).
   * **Clic derecho** en un poste: tipo, armado, poste, altura de amarre, retenidas, eliminar.
   * Conductor **verde** = vano cumple, **rojo** = no cumple (✕ marca el punto crítico con
     su despeje); línea morada = despeje mínimo; poste rojo = falla de verificación.
   * **Ctrl+Z** deshace; la barra superior da zoom y desplazamiento.
5. **Planilla de estructuras**: editable como hoja de cálculo (progresiva, tipo, armado,
   poste, altura, retenidas); recalcula al instante.
6. Resultados: *Vanos*, *Mecánico*, *Cargas*, *Verificación* (FS de postes y
   retenidas), *Metrado* y *Avisos*. **Exportar** genera DXF, Excel y memoria PDF.

Otros: *Herramientas → Configurar visor DXF (CADCraft)*; *Archivo → Importar proyecto
LineCraft (.json)*; *Archivo → Recientes*. Por línea de comandos:
`linecraft calcular proyecto.sel --out salidas`.

**Catálogos**: vienen con los postes del proyecto Capachica (CAC 12/300…) y los armados trifásicos de la Norma DGE RD 024-2003 (PS1-3, PA1-3, PA2-3, PA3-3, PR3-3, PTV-3,
retenida 3/8") marcados PENDIENTE. Complete empotramiento y distancia punta–amarre para que
la altura de amarre se calcule desde el poste, y los materiales por armado para un metrado
completo.

## 2. Topografía (TopoCraft)

Prepare el levantamiento en CSV (UTM WGS84, metros):

```
punto,este,norte,cota,descripcion
1,412606.01,8270712.24,3924.17,BM
2,412620.50,8270730.10,3921.80,TN
```

También sirve con `;`, coma decimal o sin encabezado (orden P,E,N,Z,D), o un DXF
con puntos. El eje de la línea es un DXF con una polilínea (de preferencia en la
capa `EJE`) o un CSV `este,norte`.

```bash
topocraft validar puntos.csv                       # revisa duplicados y cotas atípicas
topocraft curvas  puntos.csv --eq 1 --eje eje.dxf --out plano.dxf
topocraft perfil  puntos.csv --eje eje.dxf --out perfil.pdf perfil.xlsx perfil.dxf
```

* `--quitar-atipicos` elimina las cotas atípicas (por defecto solo se informan).
* `--max-lado 50` evita triángulos largos en los bordes del levantamiento.
* `--esc-h 2000 --esc-v 200` fija las escalas del PDF (imprimir al 100 %).

## 3. Línea primaria (LineCraft)

```bash
linecraft nuevo mi_linea          # crea proyecto.json, conductores.csv, estructuras.csv
```

1. Copie `puntos.csv` y `eje.dxf` a `mi_linea/` (o en `proyecto.json` use
   `"perfil": {"excel": "perfil.xlsx"}` con el Excel de TopoCraft).
2. Edite `proyecto.json`: conductor, hipótesis, EDS, tiro máximo, distancia
   mínima, altura de amarre, vano máximo **y la fuente de cada valor** en `fuentes`.
   Los valores de la plantilla vienen del proyecto Capachica y están marcados
   PENDIENTE: verifíquelos contra el CNE y las normas DGE.
3. Verifique `conductores.csv` con la ficha del fabricante.
4. Calcule:

```bash
linecraft calcular mi_linea/proyecto.json --out mi_linea/salidas
```

Salidas en `mi_linea/salidas/`:

| Archivo | Contenido |
|---|---|
| `perfil_linea.dxf` | Terreno, postes, conductor con flecha máxima y línea de despeje mínimo |
| `planilla_estructuras.xlsx` | Estructuras, vanos/flechas/despejes, cálculo mecánico, cargas, avisos |
| `memoria_calculo.pdf` | Memoria con datos, fuentes, fórmulas, perfil y tablas |

La consola indica **CUMPLE / NO CUMPLE** y los avisos. Para fijar los postes a
mano, llene `estructuras.csv` (incluya 0, el final y todos los vértices del eje)
y ponga `"estructuras": "estructuras.csv"` en `proyecto.json`.

## 4. Ejemplos incluidos

```bash
python examples/generar_ejemplos.py
topocraft curvas examples/colina_gaussiana.csv --eq 1 --eje examples/eje.dxf --out examples/salidas/plano.dxf
linecraft calcular examples/linecraft/proyecto.json --out examples/salidas/linecraft
```

## 5. Problemas frecuentes

| Mensaje | Solución |
|---|---|
| `Faltan columnas` | Revise el encabezado del CSV (`este`, `norte`, `cota`). |
| `No se encontró polilínea de eje` | El DXF del eje debe tener una LWPOLYLINE o POLYLINE. |
| `vértice del eje ... no tiene estructura` | En la ubicación manual incluya todos los vértices. |
| `sale del modelo de terreno` | El eje sale del área levantada: amplíe el levantamiento. |
| `topocraft` no se reconoce (Windows) | Abra una terminal nueva después de `instalar.bat`. |

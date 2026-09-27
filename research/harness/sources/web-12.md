# Web source

- URL: https://open-code.ai/es/docs/troubleshooting
- Title: Solución de Problemas de OpenCode: Errores y Desinstalación - OpenCode Docs
- Author(s): —
- Language: Spanish
- Published (UTC): —
- Captured (UTC): 2026-09-05T10:27:25.845242941+00:00
- Relevance: High — title matches query


```text
Problemas comunes y cómo resolverlos.
Para depurar problemas con OpenCode, comienza revisando los logs y los datos locales que almacena en disco.
LogsLos archivos de log se escriben en:
macOS/Linux : ~/.local/share/opencode/log/Windows : Presiona WIN+R y pega %USERPROFILE%\.local\share\opencode\logLos archivos de log se nombran con marcas de tiempo (por ejemplo, 2025-01-09T123456.log) y se conservan los 10 archivos de log más recientes.
Puedes establecer el nivel de log con la opción de línea de comandos --log-level para obtener información de depuración más detallada. Por ejemplo, opencode --log-level DEBUG.
Almacenamientoopencode almacena los datos de sesión y otros datos de la aplicación en disco en:
macOS/Linux : ~/.local/share/opencode/Windows : Presiona WIN+R y pega %USERPROFILE%\.local\share\opencodeEste directorio contiene:
auth.json - Datos de autenticación como claves de API y tokens OAuthlog/ - Logs de la aplicaciónproject/ - Datos específicos del proyecto como datos de sesión y mensajes
Si el proyecto está dentro de un repositorio Git, se almacena en ./<project-slug>/storage/Si no es un repositorio Git, se almacena en ./global/storage/DesinstalarPara desinstalar la CLI de OpenCode y eliminar sus archivos relacionados, ejecuta:
El comando muestra qué se eliminará y pide confirmación. Consulta la referencia de la CLI para conocer las opciones que permiten conservar tu configuración o los datos de la aplicación.
Para eliminar OpenCode Desktop, desinstala la aplicación mediante las herramientas de gestión de aplicaciones de tu sistema operativo.
Aplicación de escritorioOpenCode Desktop ejecuta un servidor OpenCode local (el sidecar opencode-cli) en segundo plano. La mayoría de los problemas se deben a un plugin que funciona mal, una caché corrupta o un ajuste de servidor incorrecto.
Comprobaciones rápidasCierra completamente y vuelve a iniciar la aplicación.Si la aplicación muestra una pantalla de error, haz clic en Restart y copia los detalles del error.Solo en macOS: menú OpenCode -> Reload Webview (ayuda si la interfaz está en blanco/congelada).Deshabilitar pluginsSi la aplicación de escritorio se bloquea al iniciar, se cuelga o se comporta de forma extraña, comienza deshabilitando los plugins.
Comprueba la configuración globalAbre tu archivo de configuración global y busca una clave plugin.
macOS/Linux : ~/.config/opencode/opencode.jsonc (o ~/.config/opencode/opencode.json )macOS/Linux (instalaciones antiguas): ~/.local/share/opencode/opencode.jsoncWindows : Presiona WIN+R y pega %USERPROFILE%\.config\opencode\opencode.jsoncSi tienes plugins configurados, deshabilítalos temporalmente eliminando la clave o estableciéndola en un array vacío:
Comprueba los directorios de pluginsOpenCode también puede cargar plugins locales desde disco. Muévelos temporalmente fuera del camino (o renombra la carpeta) y reinicia la aplicación de escritorio:
Si la aplicación vuelve a funcionar, vuelve a habilitar los plugins uno por uno para encontrar cuál está causando el problema.
Limpiar la cachéSi deshabilitar los plugins no ayuda (o la instalación de un plugin está atascada), limpia la caché para que OpenCode pueda reconstruirla.
Cierra OpenCode Desktop por completo.Elimina el directorio de caché:macOS : Finder -> Cmd+Shift+G -> pega ~/.cache/opencodeLinux : elimina ~/.cache/opencode (o ejecuta rm -rf ~/.cache/opencode )Windows : Presiona WIN+R y pega %USERPROFILE%\.cache\opencodeReinicia OpenCode Desktop.Solucionar problemas de conexión del servidorOpenCode Desktop puede iniciar su propio servidor local (por defecto) o conectarse a una URL de servidor que hayas configurado.
Si ves un diálogo de "Connection Failed" (o la aplicación nunca pasa de la pantalla de inicio), comprueba si hay una URL de servidor personalizada.
Limpiar la URL del servidor por defecto del escritorioDesde la pantalla de inicio, haz clic en el nombre del servidor (con el punto de estado) para abrir el selector de servidores. En la sección Default server, haz clic en Clear.
Elimina server.port / server.hostname de tu configuraciónSi tu opencode.json(c) contiene una sección server, elimínala temporalmente y reinicia la aplicación de escritorio.
Comprueba las variables de entornoSi tienes OPENCODE_PORT establecida en tu entorno, la aplicación de escritorio intentará usar ese puerto para el servidor local.
Anula OPENCODE_PORT (o elige un puerto libre) y reinicia.Linux: problemas de Wayland / X11En Linux, algunas configuraciones de Wayland pueden causar ventanas en blanco o errores del compositor.
Si estás en Wayland y la aplicación está en blanco o se bloquea, prueba a iniciarla con OC_ALLOW_WAYLAND=1 .Si eso empeora las cosas, quítalo e intenta iniciarla bajo una sesión X11 en su lugar.Windows: runtime de WebView2En Windows, OpenCode Desktop requiere el WebView2 Runtime de Microsoft Edge. Si la aplicación se abre con una ventana en blanco o no inicia, instala/actualiza WebView2 e inténtalo de nuevo.
Windows: problemas generales de rendimientoSi experimentas un rendimiento lento, problemas de acceso a archivos o problemas de terminal en Windows, prueba a usar WSL (Windows Subsystem for Linux). WSL proporciona un entorno Linux que funciona de forma más fluida con las características de OpenCode.
Las notificaciones no se muestranOpenCode Desktop solo muestra notificaciones del sistema cuando:
las notificaciones están habilitadas para OpenCode en la configuración de tu SO, yla ventana de la aplicación no tiene el foco.Restablecer el almacenamiento de la aplicación de escritorio (último recurso)Si la aplicación no inicia y no puedes limpiar la configuración desde dentro de la interfaz, restablece el estado guardado de la aplicación de escritorio.
Cierra OpenCode Desktop.Encuentra y elimina estos archivos (se encuentran en el directorio de datos de la aplicación OpenCode Desktop):opencode.settings.dat (URL del servidor por defecto del escritorio)opencode.global.dat y opencode.workspace.*.dat (estado de la interfaz, como servidores/proyectos recientes)Para encontrar el directorio rápidamente:
macOS : Finder -> Cmd+Shift+G -> ~/Library/Application Support (luego busca los nombres de archivo anteriores)Linux : busca bajo ~/.local/share los nombres de archivo anterioresWindows : Presiona WIN+R -> %APPDATA% (luego busca los nombres de archivo anteriores)Obtener ayudaSi tienes problemas con OpenCode:
Reporta problemas en GitHub La mejor manera de reportar bugs o solicitar funciones es a través de nuestro repositorio de GitHub: github.com/anomalyco/opencode/issues (opens in a new tab) Antes de crear un nuevo issue, busca entre los issues existentes para ver si tu problema ya se ha reportado.Únete a nuestro Discord Para obtener ayuda en tiempo real y participar en la discusión de la comunidad, únete a nuestro servidor de Discord: opencode.ai/discord (opens in a new tab)Problemas comunesAquí tienes algunos problemas comunes y cómo resolverlos.
OpenCode no iniciaRevisa los logs en busca de mensajes de errorPrueba a ejecutarlo con --print-logs para ver la salida en la terminalAsegúrate de tener la última versión con opencode upgradeProblemas de autenticaciónPrueba a volver a autenticarte con el comando /connect en la TUIComprueba que tus claves de API sean válidasAsegúrate de que tu red permita conexiones a la API del proveedorModelo no disponibleComprueba que te hayas autenticado con el proveedorVerifica que el nombre del modelo en tu configuración sea correctoAlgunos modelos pueden requerir un acceso o suscripciones específicasSi encuentras ProviderModelNotFoundError, lo más probable es que estés referenciando incorrectamente un modelo en algún lugar.
Los modelos deben referenciarse así: <providerId>/<modelId>
Ejemplos:
openai/gpt-4.1openrouter/google/gemini-2.5-flashopencode/kimi-k2Para averiguar a qué modelos tienes acceso, ejecuta opencode models
ProviderInitErrorSi encuentras un ProviderInitError, probablemente tengas una configuración inválida o corrupta.
Para resolverlo:
Primero, verifica que tu proveedor esté configurado correctamente siguiendo la guía de proveedoresSi el problema persiste, prueba a limpiar tu configuración almacenada:
En Windows, presiona WIN+R y elimina: %USERPROFILE%\.local\share\opencodeVuelve a autenticarte con tu proveedor usando el comando /connect en la TUI.AI_APICallError y problemas con los paquetes de proveedorSi encuentras errores de llamada a la API, esto puede deberse a paquetes de proveedor desactualizados. opencode instala dinámicamente los paquetes de proveedor (OpenAI, Anthropic, Google, etc.) según sea necesario y los almacena en caché localmente.
Para resolver problemas con los paquetes de proveedor:
Limpia la caché de paquetes de proveedor:
En Windows, presiona WIN+R y elimina: %USERPROFILE%\.cache\opencodeReinicia opencode para reinstalar los últimos paquetes de proveedorEsto forzará a opencode a descargar las versiones más recientes de los paquetes de proveedor, lo que a menudo resuelve problemas de compatibilidad con los parámetros de los modelos y los cambios de API.
Copiar/pegar no funciona en LinuxLos usuarios de Linux necesitan tener instalada una de las siguientes utilidades de portapapeles para que funcione la funcionalidad de copiar/pegar:
Para sistemas X11:
Para sistemas Wayland:
Para entornos headless:
opencode detectará si estás usando Wayland y preferirá wl-clipboard; de lo contrario, intentará encontrar herramientas de portapapeles en este orden: xclip y xsel.
```

# Español (es). Cada mensaje debe existir en todos los archivos de idioma (locales/*/main.ftl)
# (lo comprueban las pruebas unitarias de wpg-i18n).

app-name = Web Packet Generator
app-tagline = Generador de paquetes de alto rendimiento para pruebas de red, controlado desde su navegador.

## Servidor / API
health-ok = El servicio funciona correctamente
server-listening = Escuchando en { $addr }
server-placeholder-ui = La interfaz web aún no se ha compilado. Ejecute `trunk build` en crates/wpg-web.

## Errores
error-not-implemented = `{ $command }` aún no está implementado.
error-not-found = No encontrado
error-internal = Error interno del servidor

## Interfaz de usuario
ui-language = Idioma
ui-protocols = Protocolos

## Diagnósticos (wpg-core `diag::keys`; los nombres de los argumentos forman parte del contrato)
diag-field-out-of-range = El campo `{ $field }` = { $value } está fuera del rango { $min }..{ $max }.
diag-field-not-equal = El campo `{ $field }` = { $value } difiere del valor esperado { $expected }.
diag-field-forbidden-value = El campo `{ $field }` tiene un valor desaconsejado: { $value }.
diag-length-too-short = `{ $span }` mide { $len } bytes; se requieren al menos { $min }.
diag-length-too-long = `{ $span }` mide { $len } bytes; se permiten como máximo { $max }.
diag-length-mismatch = El campo de longitud `{ $field }` = { $value } difiere de la longitud calculada { $expected }.
diag-count-mismatch = El campo de recuento `{ $field }` = { $value } difiere del número real de elementos { $expected }.
diag-fill-inexact = No se puede alcanzar exactamente `{ $field }` con elementos de relleno: faltan { $len } octeto(s).
diag-checksum-mismatch = El campo de suma de comprobación `{ $field }` = { $value } difiere de la suma calculada { $expected }.
diag-next-protocol-mismatch = El campo `{ $field }` = { $value } no anuncia la capa siguiente `{ $next }` (se esperaba { $expected }).
diag-auto-overflow = El valor automático { $value } de `{ $field }` no cabe en { $bits } bits.
diag-length-not-aligned = La longitud { $len } de `{ $field }` no es múltiplo de { $unit } bytes.
diag-no-pseudo-header = La suma de comprobación `{ $field }` necesita una pseudocabecera, pero no hay ninguna capa IP debajo.
diag-not-implemented = `{ $field }`: { $what } aún no está implementado.
diag-truncated = La capa se truncó intencionadamente a { $len } bytes.
diag-short-header = La entrada termina dentro de la cabecera { $protocol } en el desplazamiento { $offset }.
diag-length-exceeds-data = El campo de longitud `{ $field }` = { $len } supera los { $available } bytes disponibles.
diag-trailing-bytes = Quedan { $len } bytes después del final del paquete.
diag-unsupported-field = El analizador aún no puede decodificar el campo `{ $field }` de { $protocol }.

## Descripciones de campos de protocolo (wpg-protocols `desc = "..."`)
arp-hlen = Longitud de la dirección de hardware en bytes (6 para Ethernet).
arp-htype = Tipo de hardware (1 = Ethernet).
arp-oper = Operación (1 = solicitud, 2 = respuesta).
arp-plen = Longitud de la dirección de protocolo en bytes (4 para IPv4).
arp-ptype = Tipo de protocolo (EtherType, 0x0800 = IPv4).
arp-sha = Dirección de hardware del emisor.
arp-spa = Dirección de protocolo del emisor.
arp-tha = Dirección de hardware de destino.
arp-tpa = Dirección de protocolo de destino.
icmp-code = Código del mensaje; su significado depende del tipo.
icmp-identifier = Identificador para emparejar solicitudes y respuestas de eco.
icmp-rest = Campo "resto de la cabecera" de 4 bytes, específico del tipo.
icmp-sequence = Número de secuencia para emparejar solicitudes y respuestas de eco.
icmp-type = Tipo de mensaje.
ipv6-fragment-offset = Desplazamiento de este fragmento en unidades de 8 bytes.
ipv6-more-fragments = Indicador de más fragmentos (1 = siguen más fragmentos).
ipv6-options = Opciones TLV en bruto, rellenadas hasta un múltiplo de 8 bytes.
ipv6-routing-data = Datos de enrutamiento específicos del tipo.
ipv6-routing-type = Tipo de cabecera de enrutamiento (p. ej., 2 = Mobile IPv6, 4 = SRH).
ipv6-segments-left = Número de segmentos de ruta restantes.
srh-segments-left = Segments Left: índice (en la lista de segmentos) del siguiente segmento que se visitará; 0 en el último.
srh-last-entry = Last Entry: índice del último elemento de la lista de segmentos, es decir, número de segmentos - 1 (Auto).
srh-flags = Indicadores (RFC 8754 no define ninguno; 0).
srh-tag = Tag: marca los paquetes como parte de una clase o grupo (0 = sin uso).
srh-segment-list = Lista de segmentos (SID) en orden inverso de la ruta: el elemento 0 es el último segmento y el último elemento, el primero. Mientras Segments Left no sea cero, las sumas de comprobación de capas superiores usan el elemento 0 como destino.
srh-tlvs = TLV opcionales (tipo de 8 bits, longitud de 8 bits): Pad1, PadN, HMAC (el valor HMAC es Manual). Después, la cabecera se rellena con octetos Pad1 hasta un múltiplo de 8 (Auto).
ndp-destination = Dirección de destino de una redirección.
ndp-managed = Indicador de configuración de direcciones administrada (M).
ndp-options = Opciones NDP en bruto (TLV).
ndp-other = Indicador de otra configuración (O).
ndp-override-flag = Indicador de anulación (O).
ndp-router-flag = Indicador de router (R).
ndp-solicited-flag = Indicador de solicitado (S).

# --- Descripciones de los campos de los protocolos P0 (Ethernet, VLAN, IPv4 / IPv6, TCP / UDP, carga útil) ---
eth-dst = Dirección MAC de destino.
eth-src = Dirección MAC de origen.
eth-ethertype = EtherType de la capa siguiente (Auto desde la capa superior; 0x88b5 = experimental local si no se conoce ninguna). Un valor hasta 0x05ff es una longitud IEEE 802.3.
dot3-length = Longitud de los datos LLC en octetos (Auto; como máximo 1500).
llc-dsap = Punto de acceso al servicio de destino (Auto: 0xaa = SNAP, 0xfe = OSI, 0x42 = spanning tree).
llc-ssap = Punto de acceso al servicio de origen (Auto, normalmente igual al DSAP).
llc-control = Campo de control; 0x03 = información no numerada (UI).
snap-oui = Identificador único de organización (OUI); 0 significa que el identificador de protocolo es un EtherType.
snap-pid = Identificador de protocolo (un EtherType cuando el OUI es 0; Auto desde la capa siguiente).
vlan-pcp = Priority code point (clase de servicio 802.1p, 0 a 7).
vlan-dei = Indicador de elegibilidad de descarte (DEI).
vlan-vid = Identificador de VLAN (1 a 4094; 0 = solo etiqueta de prioridad, 4095 está reservado).
vlan-ethertype = EtherType de la capa después de la etiqueta (Auto; 0x8100 para otra etiqueta 802.1Q).
dot1ad-ethertype = EtherType de la capa después de la etiqueta S (Auto; normalmente 0x8100 para la etiqueta C 802.1Q).
ipv4-version = Versión IP, siempre 4.
ipv4-ihl = Longitud de la cabecera en palabras de 32 bits, 5 a 15 (Auto, incluye las opciones).
ip-dscp = Differentiated services code point (clase QoS, 0 a 63).
ip-ecn = Notificación explícita de congestión (0 = no ECT, 1 / 2 = ECT, 3 = CE).
ipv4-total-length = Longitud total de cabecera y carga útil en octetos (Auto).
ipv4-identification = Identificación compartida por los fragmentos de un mismo datagrama.
ipv4-flags = Indicadores: DF (no fragmentar) y MF (más fragmentos); el bit superior está reservado.
ipv4-frag-offset = Desplazamiento de este fragmento en unidades de 8 octetos.
ipv4-ttl = Tiempo de vida; se decrementa en cada router, 0 nunca se envía.
ipv4-protocol = Número de protocolo de la capa siguiente (Auto; 253 = experimental si no se conoce ninguna).
ipv4-header-checksum = Suma de comprobación de Internet de la cabecera (Auto).
ipv4-src = Dirección IPv4 de origen.
ipv4-dst = Dirección IPv4 de destino.
ipv4-options = Opciones IPv4 como TLV (EOL, NOP, seguridad, registro de ruta, marca de tiempo...).
ipv4-padding = Relleno con ceros de las opciones hasta un límite de 32 bits (Auto).
udp-src-port = Puerto de origen.
udp-dst-port = Puerto de destino; selecciona la capa siguiente cuando es Auto (53 = DNS, 67 / 68 = DHCP, 4789 = VXLAN...).
udp-length = Longitud de cabecera y carga útil en octetos (Auto).
udp-checksum = Suma de comprobación de Internet con la pseudocabecera IPv4 / IPv6 (Auto; 0 pasa a ser 0xffff).
tcp-src-port = Puerto de origen.
tcp-dst-port = Puerto de destino; selecciona la capa siguiente (179 = BGP, 646 = LDP, 3868 = Diameter, 5060 = SIP).
tcp-seq = Número de secuencia del primer octeto de datos (número de secuencia inicial con SYN).
tcp-ack = Número de acuse de recibo, válido cuando el indicador ACK está activo.
tcp-data-offset = Longitud de la cabecera en palabras de 32 bits, 5 a 15 (Auto, incluye las opciones).
tcp-reserved = Bits reservados, 0.
tcp-flags = Indicadores de control: CWR, ECE, URG, ACK, PSH, RST, SYN, FIN (SYN por defecto).
tcp-window = Ventana de recepción en octetos (escalada por la opción window scale).
tcp-checksum = Suma de comprobación de Internet con la pseudocabecera IPv4 / IPv6 (Auto).
tcp-urgent-ptr = Puntero urgente, válido cuando el indicador URG está activo.
tcp-options = Opciones TCP como TLV: EOL, NOP, MSS, window scale, SACK permitted, SACK, timestamps.
tcp-padding = Relleno con ceros de las opciones hasta un límite de 32 bits (Auto).
payload-mode = Cómo se genera la carga útil: fixed (datos tal cual), pattern (datos repetidos hasta la longitud), random (longitud octetos a partir de la semilla) o counter (datos con un contador por paquete incrustado).
payload-data = Octetos de la carga útil; en modo fixed se recortan o se extienden con ceros hasta la longitud si no es cero, en modo pattern la unidad repetida (vacía = 00 01 02 ...).
payload-length = Longitud de la carga útil en octetos; 0 conserva la longitud de los datos (modo fixed / counter).
payload-seed = Semilla del generador pseudoaleatorio en modo random.
payload-counter-offset = Desplazamiento en octetos del contador incrustado en modo counter.
payload-counter-width = Anchura del contador incrustado: 4 u 8 octetos, big endian.
payload-counter-value = Valor del contador escrito por paquete por la expansión de flujos (modo counter).
raw-data = Octetos sin procesar hasta el final de la trama.
ipv6-version = Versión IP, siempre 6.
ipv6-flow-label = Etiqueta de flujo (20 bits) que identifica un flujo para los routers; 0 = ninguna.
ipv6-payload-length = Longitud de todo lo que sigue a esta cabecera, incluidas las cabeceras de extensión (Auto).
ipv6-next-header = Número de protocolo de la cabecera siguiente (Auto; 253 = experimental si no se conoce ninguna).
ipv6-hop-limit = Límite de saltos; se decrementa en cada router, 0 nunca se envía.
ipv6-src = Dirección IPv6 de origen.
ipv6-dst = Dirección IPv6 de destino.
ipv6-ext-next-header = Número de protocolo de la cabecera que sigue a esta cabecera de extensión (Auto).
ipv6-hdr-ext-len = Longitud de esta cabecera de extensión en unidades de 8 octetos, sin contar los primeros 8 octetos (Auto).
ipv6-ext-padding = Relleno con ceros (Pad1) de la cabecera de extensión hasta un múltiplo de 8 octetos (Auto).
field-reserved = Reservado, 0.
ipv6-frag-identification = Identificación compartida por los fragmentos de un mismo paquete.
icmp-checksum = Suma de comprobación de Internet de cabecera y carga útil (Auto).
icmpv6-checksum = Suma de comprobación de Internet con la pseudocabecera IPv6 sobre cabecera y carga útil (Auto).
ndp-cur-hop-limit = Límite de saltos que deben usar los hosts (0 = sin especificar).
ndp-router-lifetime = Tiempo de vida como router por defecto en segundos (0 = no es router por defecto).
ndp-reachable-time = Tiempo de alcanzabilidad en milisegundos (0 = sin especificar).
ndp-retrans-timer = Temporizador de retransmisión en milisegundos (0 = sin especificar).
ndp-target = Dirección objetivo.

## wpg ifaces
ifaces-col-name = NOMBRE
ifaces-col-index = ÍND
ifaces-col-mac = MAC
ifaces-col-ipv4 = IPV4
ifaces-col-ipv6 = IPV6
ifaces-col-mtu = MTU
ifaces-col-state = ESTADO
ifaces-col-speed = VELOCIDAD
ifaces-col-driver = CONTROLADOR
ifaces-state-up = activa
ifaces-state-down = inactiva
ifaces-state-no-carrier = sin portadora
ifaces-state-loopback = loopback
ifaces-none = No se encontraron interfaces de red.
ifaces-error = No se pudieron listar las interfaces de red: { $error }

## wpg doctor
doctor-title = wpg doctor: comprobación del entorno
doctor-summary = { $ok } correcto(s), { $warn } advertencia(s), { $fail } fallo(s)
doctor-skip = No aplicable en este sistema operativo
doctor-caps-ok = Los sockets sin procesar (raw) están disponibles
doctor-caps-missing = Faltan privilegios para transmitir paquetes sin procesar (CAP_NET_RAW / root / Administrador)
doctor-caps-npcap-user = Sin privilegios elevados, pero Npcap permite capturar y enviar a usuarios que no son administradores
doctor-caps-npcap-admin = Npcap está restringido a administradores; ejecute wpg desde una terminal con privilegios elevados para enviar
doctor-fix-caps = Conceda el privilegio: { $hint }
doctor-iface-ok = Al menos una interfaz de red utilizable está activa
doctor-iface-none = Ninguna interfaz que no sea de loopback está activa con portadora
doctor-iface-error = No se pudieron enumerar las interfaces de red
doctor-fix-iface = Revise las interfaces: { $hint }
doctor-af-xdp-ok = El kernel admite AF_XDP (5.10 o posterior)
doctor-af-xdp-old = El kernel es demasiado antiguo para AF_XDP (se requiere 5.10 o posterior); el backend afpacket sigue funcionando
doctor-tpacket-v3-ok = El kernel admite TPACKET_V3
doctor-tpacket-v3-old = El kernel no admite TPACKET_V3; afpacket recurre a sendmmsg
doctor-hugepages-ok = Las hugepages están configuradas
doctor-hugepages-none = No hay hugepages configuradas (solo son necesarias para el backend DPDK)
doctor-fix-hugepages = Reserve hugepages y vincule la NIC a DPDK: { $hint }
doctor-npcap-ok = Npcap está instalado
doctor-npcap-missing = Npcap no está instalado; es necesario para transmitir en Windows
doctor-npcap-no-dll = Npcap parece estar instalado, pero no se encontró wpcap.dll ni en System32\Npcap ni en System32
doctor-fix-npcap = Instale (o reinstale) Npcap y vuelva a ejecutar wpg doctor: { $hint }
doctor-pcaplib-ok = libpcap está disponible
doctor-pcaplib-missing = No se encontró libpcap; no se puede usar el backend pcap
doctor-fix-pcaplib = Instale libpcap: { $hint }

## wpg template lint (claves `LintIssue` de wpg-template)
lint-level-error = error
lint-level-warning = advertencia
lint-schema-version = Versión de esquema no admitida { $value } (admitida: { $expected }).
lint-missing-name = `meta.name` está vacío.
lint-no-layers = La plantilla no tiene capas.
lint-unknown-protocol = Protocolo desconocido `{ $protocol }`.
lint-unknown-field = El protocolo `{ $protocol }` no tiene el campo `{ $field }`.
lint-invalid-value = Valor no válido para `{ $field }`: { $value }
lint-invalid-generator = Generador no válido para `{ $field }`: { $value }
lint-unknown-variable = Variable desconocida `{ $value }`.
lint-missing-variable = La variable `{ $value }` no tiene valor predeterminado y no se indicó ningún valor.
lint-unused-variable = La variable `{ $value }` nunca se usa.
lint-duplicate-variable = La variable `{ $value }` se declara más de una vez.
lint-var-syntax = Marcador de variable sin cerrar en `{ $value }`.
lint-override-unknown = Sustitución de variable desconocida `{ $value }`.
lint-field-and-generator = `{ $field }` tiene a la vez un valor fijo y un generador; prevalece el generador.
lint-next-protocol-manual = `{ $next }` no está en la tabla de vinculación de `{ $protocol }`; `{ $field }` será Manual.
lint-unknown-order = `{ $value }` en el orden del flujo no tiene generador.
lint-invalid-stream = Configuración de flujo no válida: { $value }
lint-build-error = No se puede construir el paquete: { $value }

## wpg template / wpg render
template-lint-ok = { $value }: sin problemas
template-lint-load-error = { $value }: { $error }
template-lint-summary = { $fail } error(es), { $warn } advertencia(s) en { $ok } archivo(s)
template-list-empty = No se encontraron plantillas en { $value }.
template-show-hex = Volcado hexadecimal ({ $len } bytes)
template-show-layout = Estructura
render-done = Se escribieron { $len } paquetes en { $value }.
render-unbounded = El flujo no tiene límite; indique --count N.
cli-bad-var = --var `{ $value }` no válido; se esperaba NOMBRE=VALOR.

## wpg send / wpg bench (progreso, resumen, errores)
tx-progress = [{ $t } s] { $packets } paq.  { $pps }pps  { $bps }bps  descartados { $dropped }
tx-progress-rate = tasa { $ratio }%
tx-progress-paused = (en pausa)
tx-progress-scheduled = (programado, empieza en { $secs } s)
tx-interrupted = Interrumpido, deteniendo...
tx-ctrlc-warning = Advertencia: no se puede instalar el controlador de Ctrl-C: { $error }
tx-open-failed = No se puede abrir { $backend } en `{ $iface }`: { $error }
tx-start-failed = No se puede iniciar el trabajo: { $error }
tx-available-backends = Backends disponibles: { $value }
tx-iface-required = Se requiere --iface para el backend { $backend }.
tx-iface-required-pcapfile = Se requiere --iface <ruta de salida> para el backend pcapfile.
tx-bad-workers = --workers debe ser al menos 1.
tx-workers-unsupported = El backend { $backend } admite un único worker; use --workers 1.
tx-pin-cores-short = Advertencia: --pin-cores indica { $count } núcleo(s) para { $workers } workers; el resto no se fija.
tx-json-error = No se puede escribir el JSON: { $error }
send-bad-rate = --rate no válido: { $error }
send-bad-duration = --duration no válido: { $error }
send-bad-ramp-up = --ramp-up no válido: { $error }
send-bad-start-at = --start-at no válido: { $error }
send-link-speed-unknown = No se puede determinar la velocidad de enlace de `{ $iface }` para `{ $value }%`; indique una tasa absoluta como `1Gbps`.
send-bad-plan = --plan debe ser auto, prerender u onthefly.
send-stream-error = Error de flujo: { $error }
send-render-error = Error de renderizado: { $error }
send-starting = Enviando en { $iface } mediante { $backend }{ $detail }: tasa { $rate }, ráfaga { $burst }, { $limit }, { $workers } worker(s), plan { $strategy } ({ $reason })
send-limit-count = { $count } paquetes
send-limit-count-or-duration = { $count } paquetes o { $duration }
send-limit-duration = durante { $duration }
send-limit-forever = hasta Ctrl-C
send-ramp-up = Rampa: de 0 a { $rate } en { $duration }
send-scheduled = Inicio programado a las { $time } (en { $secs } s); Ctrl-C cancela.
summary-state = estado     { $value }
summary-state-finished = finalizado ({ $reason })
summary-state-failed = fallido: { $error }
summary-reason-count-reached = recuento alcanzado
summary-reason-duration-elapsed = duración agotada
summary-reason-source-exhausted = flujo agotado
summary-reason-stopped = detenido
summary-backend = backend    { $backend }{ $detail }
summary-workers = workers    { $workers }
summary-packets = paquetes   { $value }
summary-bytes = bytes      { $value }
summary-dropped = descart.   { $value }
summary-elapsed = tiempo     { $secs } s
summary-average = promedio   { $pps }pps  { $bps }bps (L2)  { $l1 }bps (L1)
summary-accuracy = precisión  objetivo { $target }{ $unit }  logrado { $ratio }%  jitter máx. { $max }  jitter medio { $mean }
summary-stream = flujo { $index }   { $packets } paq.  { $bytes } bytes
summary-worker = worker { $index }   { $packets } paq.  { $bytes } bytes{ $core }
summary-worker-core = , núcleo { $core }
summary-warning = aviso      { $error }
summary-unsigned = sin firma  { $value } (sin espacio para una firma)

## wpg capture (recepción / verificación)
rx-progress = [{ $t } s] { $packets } paq.  { $pps }pps  { $bps }bps  coincidentes { $matched } ({ $mpps }pps)  descartados { $dropped }
capture-starting = Capturando en { $iface } mediante { $backend }, filtro { $filter } (Ctrl-C detiene)
capture-filter-all = (todas las tramas)
capture-iface-required-pcapfile = Se requiere --iface <archivo de captura> para el backend pcapfile.
capture-bad-filter = --filter no válido: { $error }
capture-running = en ejecución
capture-reason-end-of-input = fin de la entrada
capture-packet = #{ $index }  { $time }  { $len } B  { $layers }{ $signature }
capture-packet-signature = { "  " }[wpg flujo { $stream } sec { $seq }]
capture-summary-matched = coincid.   { $packets } paq.  { $bytes } bytes
capture-summary-average = promedio   { $pps }pps  { $bps }bps  coincidentes { $mpps }pps  { $mbps }bps
capture-summary-saved = guardados  { $count } paq. en { $path }
capture-summary-signed = firmados   { $signed } con firma, { $unsigned } sin firma
capture-summary-stream = flujo { $stream } recibidos { $received }  sec { $first }-{ $last }  perdidos { $lost } ({ $ratio }%)  reordenados { $reordered }  duplicados { $duplicates }
capture-summary-latency = { "             " }latencia mín. { $min }  media { $avg }  máx. { $max }  jitter { $jitter }

bench-iface = interfaz   { $iface }
bench-worker = worker { $index }   { $detail }
bench-frame = trama      { $len } B (+4 FCS)  lote { $batch }
bench-template = plantilla  { $value } (sobre la marcha, { $mode })
bench-mode-patch = modificado in situ
bench-mode-rebuild = reconstrucción completa por trama
bench-rate = tasa       { $pps }pps
bench-bits = { $bps }bps (L2)  { $l1 }bps (L1)
bench-bad-duration = --duration debe ser un número no negativo de segundos.
bench-failed = La prueba de rendimiento falló: { $error }
import-done = Se importaron { $len } paquete(s) en { $count } plantilla(s): { $value }
import-read-error = { $value }: { $error }
import-write-error = { $value }: { $error }
import-note = { $value }
import-note-packet = paquete { $index }: { $value }
import-packet-problems = el paquete { $index } tiene { $count } problema(s) de análisis
import-packet-skipped = el paquete { $index } no se pudo decodificar y se omitió
import-not-reproducible = La plantilla importada no reconstruye los octetos capturados; la trama se guardó como datos en bruto.
import-mixed-stacks = { $count } paquete(s) tienen una pila de capas distinta de la del primero; no se infirió ningún generador.
import-generator-approximate = { $value } solo se aproxima a los valores capturados.
import-generators-dropped = Los generadores inferidos producían un flujo no válido y se descartaron.
template-export-none = No se indicó ninguna plantilla para exportar.
template-export-not-found = Plantilla no encontrada: { $value }
template-export-done = Se exportaron { $count } plantilla(s) a { $value }.
template-export-error = { $value }: { $error }
template-import-done = Se extrajeron { $count } plantilla(s) de { $value } en { $detail }.
template-import-exists = { $value }: ya existe (use --force para sobrescribir)
template-import-error = { $value }: { $error }
template-migrate-outdated = { $value }: esquema { $found }, hay que migrarlo a { $expected }
template-migrate-done = { $value }: migrado del esquema { $found } a { $expected }
template-migrate-error = { $value }: { $error }
template-migrate-summary = { $count } migrado(s), { $ok } ya en el esquema { $expected }, { $fail } error(es)
template-migrate-check-summary = { $count } por migrar, { $ok } ya en el esquema { $expected }, { $fail } error(es)

## Errores de la API (wpg-server)
api-error-not-found = No se encontró { $what }.
api-error-invalid-body = Cuerpo de la solicitud no válido: { $error }
api-error-invalid-query = Parámetro de la solicitud no válido: { $error }
api-error-invalid-template = Plantilla no válida: { $error }
api-error-invalid-hex = Entrada hexadecimal no válida: { $error }
api-error-parse-failed = No se pudieron decodificar los bytes: { $error }
api-error-render-failed = Error de renderizado: { $error }
api-error-count-range = `count` debe estar entre 1 y { $max } (se recibió { $error }).
api-error-read-only = La plantilla { $id } pertenece a una biblioteca de solo lectura; duplíquela para editarla.
api-error-policy-iface = La política del servidor no permite enviar por { $iface } (permitidas: { $allowed }).
api-error-policy-rate = La tasa { $rate } supera el máximo permitido por la política del servidor ({ $max }).
api-error-policy-unlimited = La política del servidor no permite una tasa ilimitada (máximo { $max }).
api-error-invalid-state = El trabajo { $id } está en estado { $state }; no se puede { $action }.
api-error-job-spec = Trabajo no válido: { $error }
api-error-backend = Error del backend de transmisión: { $error }
api-error-capture-spec = Captura no válida: { $error }
api-error-capture-filter = Filtro de captura no válido: { $error }
api-error-capture-backend = Error del backend de recepción: { $error }
api-error-capture-state = La captura { $id } está en estado { $state }; no se puede { $action }.
api-error-internal = Error interno del servidor: { $error }
auth-error-unauthorized = Inicie sesión.
auth-error-bad-credentials = Nombre de usuario o contraseña incorrectos.
auth-error-setup-required = Aún no se ha definido la contraseña de administrador: complete primero la configuración inicial.
auth-error-already-configured = La contraseña de administrador ya se ha definido.
auth-error-rate-limited = Demasiados inicios de sesión fallidos. Vuelva a intentarlo en { $secs } segundos.
auth-error-csrf = Falta el token CSRF o no es válido (cabecera X-CSRF-Token).
auth-error-origin = No se permiten conexiones WebSocket desde otro origen.
auth-error-setup-remote = La contraseña de administrador solo puede definirse desde el propio servidor (localhost).
auth-error-password-mismatch = Las contraseñas no coinciden.
auth-error-password-short = La contraseña debe tener al menos { $min } caracteres.
lint-parse-error = No se puede analizar la plantilla: { $error }

## Configuración (wpg.toml)
config-issue-file-missing = archivo no encontrado
config-issue-bad-url = debe empezar por http:// o https://
config-issue-not-argon2 = no parece un hash argon2 ($argon2id$...)
config-issue-password-both = se han definido password_hash y password_file; prevalece password_hash
config-issue-zero = debe ser mayor que 0
config-issue-exposed-without-password = escucha en una dirección que no es de loopback sin contraseña
config-issue-dir-missing = directorio no encontrado
config-issue-protocols = no se pueden cargar los protocolos definidos por el usuario
protocols-ext-loaded = se cargaron { $count } protocolo(s) definido(s) por el usuario
protocols-ext-error = no se pueden cargar los protocolos definidos por el usuario: { $file }: { $error }
config-issue-max-rate-percent = un porcentaje del enlace no puede ser un máximo para todo el servidor; use pps o bps
config-issue-bad-rate = tasa no válida
config-issue-unknown-backend = backend desconocido
config-issue-backend-unavailable = backend no disponible en esta compilación / en este sistema operativo
config-issue-bad-log-level = filtro de registro no válido
config-level-error = error
config-level-warning = advertencia
config-issue-line = { $level }: { $key }: { $message }{ $detail }
config-load-error = Error de configuración: { $error }
config-init-written = Se escribió { $path }. Edítelo y compruébelo con `wpg config validate`.
config-init-exists = { $path } ya existe (use --force para sobrescribir).
config-validate-ok = Configuración correcta ({ $source }).
config-validate-summary = { $errors } error(es), { $warnings } advertencia(s) ({ $source }).
config-source-defaults = valores predeterminados integrados y entorno
config-password-prompt = Nueva contraseña de administrador:{ " " }
config-password-confirm = Repita la contraseña:{ " " }
config-password-mismatch = Las contraseñas no coinciden.
config-password-short = La contraseña debe tener al menos { $min } caracteres.
config-password-written = Se guardó el hash de la contraseña en { $path } (auth.password_hash). Reinicie `wpg serve` para aplicarlo.

## wpg serve
serve-banner-title = { $app } { $version }
serve-banner-listen = Escuchando en  { $url }/
serve-banner-docs = Docs de la API { $url }/api/docs/
serve-banner-config = Configuración  { $source }
serve-banner-data = Dir. de datos  { $path }
serve-banner-library = Biblioteca     { $path }{ $state }
serve-banner-user-dir = Plantillas usu. { $path }
serve-banner-templates = Plantillas     { $count }
serve-banner-backends = Backends       { $list }
serve-banner-missing = { " " }(no existe)
serve-banner-dev = Modo de desarrollo: CORS permitido para { $origins }
serve-banner-no-password = No hay ninguna contraseña configurada y el servidor escucha en { $url } (--allow-insecure): cualquiera que pueda acceder a él puede definir la contraseña de administrador y enviar paquetes. Defina una contraseña (wpg config set-password) lo antes posible.
serve-failed = Error del servidor: { $error }
serve-log-error = No se puede configurar el registro: { $error }
serve-banner-tls = TLS            { $path }
serve-banner-tls-self-signed = TLS            { $path } (autofirmado)
serve-banner-fingerprint = SHA-256        { $fingerprint }
serve-banner-auth = Inicio sesión  { $state }
serve-banner-auth-setup = aún sin contraseña: defínala en la pantalla de configuración inicial
serve-banner-auth-configured = contraseña de administrador desde { $source }
serve-banner-auth-database = contraseña de administrador definida en la pantalla de configuración inicial
serve-banner-setup-local = Aún no se ha definido la contraseña de administrador. Abra { $url } en un navegador de este equipo para definirla.
serve-refuse-insecure = No se escuchará en { $addr }: no hay contraseña de administrador configurada. Use una dirección de loopback (127.0.0.1), defina una contraseña (--password-file, WPG_PASSWORD, wpg config set-password) o indique --allow-insecure.
serve-password-arg-warning = Advertencia: --password es visible para otros usuarios en la lista de procesos; es preferible usar --password-file, WPG_PASSWORD o `wpg config set-password`.

## Interfaz web (wpg-web). La prueba `ui_keys_exist` de crates/wpg-web comprueba las claves.
ui-lang-ja = 日本語
ui-lang-en = English
ui-lang-zh-cn = 简体中文
ui-lang-zh-tw = 繁體中文
ui-lang-ko = 한국어
ui-lang-de = Deutsch
ui-lang-fr = Français
ui-lang-es = Español
ui-loading = Cargando…
ui-close = Cerrar
ui-cancel = Cancelar
ui-delete = Eliminar
ui-save = Guardar
ui-refresh = Actualizar
ui-retry = Reintentar
ui-yes = Sí
ui-no = No
ui-unlimited = Ilimitado
ui-help = Atajos de teclado
ui-help-title = Atajos de teclado
ui-skip-to-content = Ir al contenido
ui-not-found = Esta página no existe.
ui-back-to-dashboard = Volver al panel
ui-view-only = Pantalla estrecha: solo lectura. La edición está disponible en pantallas más anchas.
ui-server-unreachable = No se puede acceder al servidor. ¿Se está ejecutando `wpg serve`?
ui-error-http = El servidor respondió con HTTP { $id }.
ui-error-network = Error de red: { $error }
ui-error-decode = Respuesta inesperada del servidor: { $error }
ui-nav-label = Navegación principal
ui-nav-dashboard = Panel
ui-nav-builder = Editor
ui-nav-jobs = Trabajos
ui-nav-templates = Plantillas
ui-nav-rx = Recepción
ui-nav-settings = Ajustes
ui-shortcut-hint = muestra los atajos de teclado
ui-shortcut-dashboard = Ir al panel
ui-shortcut-builder = Ir al editor
ui-shortcut-jobs = Ir a trabajos
ui-shortcut-templates = Ir a plantillas
ui-shortcut-rx = Ir a recepción
ui-shortcut-settings = Ir a ajustes
ui-shortcut-new-job = Nuevo trabajo
ui-shortcut-logs = Mostrar / ocultar el registro del servidor
ui-shortcut-help = Mostrar esta ayuda
ui-shortcut-escape = Cerrar el cuadro de diálogo
ui-ws-connecting = Conectando…
ui-ws-open = En vivo
ui-ws-closed = Desconectado (reintentando)
ui-theme = Tema
ui-theme-light = Claro
ui-theme-dark = Oscuro
ui-theme-system = Sistema
ui-theme-to-dark = Cambiar al tema oscuro
ui-theme-to-light = Cambiar al tema claro
ui-logout = Cerrar sesión
ui-logs-toggle = Registro del servidor
ui-logs-title = Registro del servidor
ui-logs-level = Nivel
ui-logs-follow = Seguir
ui-logs-clear = Borrar el registro
ui-logs-count = { $count } líneas
ui-logs-empty = Aún no hay líneas de registro.
ui-login-title = Iniciar sesión
ui-login-submit = Iniciar sesión
ui-login-busy = Iniciando sesión…
ui-login-wrong = Contraseña incorrecta.
ui-login-rate-limited = Demasiados intentos. Vuelva a intentarlo en { $secs } s.
ui-login-wait = Espere { $secs } s
ui-password = Contraseña
ui-password-confirm = Confirmar contraseña
ui-setup-title = Definir la contraseña de administrador
ui-setup-intro = Este servidor aún no tiene contraseña. Elija una para proteger la interfaz web y la API.
ui-setup-hint = Al menos { $min } caracteres.
ui-setup-too-short = La contraseña debe tener al menos { $min } caracteres.
ui-setup-mismatch = Las contraseñas no coinciden.
ui-setup-submit = Definir contraseña
ui-dash-totals = Totales
ui-dash-active-jobs = Trabajos activos
ui-dash-running-jobs = Enviando
ui-dash-total-pps = pps totales
ui-dash-total-bps = Tasa de bits total
ui-dash-templates = Plantillas
ui-dash-uptime = Tiempo de actividad del servidor
ui-dash-running = Trabajos en ejecución
ui-dash-no-running = No hay ningún trabajo en ejecución.
ui-dash-doctor = Autodiagnóstico
ui-dash-ifaces = Interfaces
ui-doctor-ok = Todas las comprobaciones son correctas
ui-doctor-warn = Advertencias
ui-doctor-fail = Se encontraron problemas
ui-doctor-counts = { $ok } OK · { $warn } advertencias · { $fail } fallos
ui-doctor-details = Informe completo
ui-doctor-status = Estado
ui-doctor-check = Comprobación
ui-doctor-detail = Detalle / solución
ui-doctor-status-ok = OK
ui-doctor-status-warn = Advertencia
ui-doctor-status-fail = Fallo
ui-doctor-status-skip = Omitido
ui-iface-name = Nombre
ui-iface-state = Estado
ui-iface-mac = MAC
ui-iface-addrs = Direcciones
ui-iface-mtu = MTU
ui-iface-speed = Velocidad
ui-iface-allowed = Permitida para TX
ui-iface-up = Activa
ui-iface-down = Inactiva
ui-iface-no-carrier = Sin portadora
ui-stat-pps = Paquetes/s
ui-stat-bps = Tasa de bits (L2)
ui-stat-l1-bps = Tasa de bits (L1)
ui-stat-packets = Paquetes
ui-stat-bytes = Bytes
ui-stat-dropped = Descartados
ui-stat-ratio = Tasa lograda
ui-stat-elapsed = Transcurrido
ui-spark-pps = Paquetes por segundo, últimos 30 segundos
ui-state-created = Creado
ui-state-scheduled = Programado
ui-state-running = En ejecución
ui-state-paused = En pausa
ui-state-finished = Finalizado
ui-state-failed = Fallido
ui-state-unknown = Desconocido
ui-reason-count-reached = Número de paquetes alcanzado
ui-reason-duration-elapsed = Duración agotada
ui-reason-source-exhausted = Todos los paquetes enviados
ui-reason-stopped = Detenido
ui-jobs-new = Nuevo trabajo
ui-jobs-tab-live = Actuales
ui-jobs-tab-history = Historial
ui-jobs-empty = No hay trabajos.
ui-jobs-streams = Flujos
ui-jobs-template = Plantilla
ui-jobs-choose-template = Elija una plantilla…
ui-jobs-weight = Peso
ui-jobs-add-stream = Añadir flujo
ui-jobs-remove-stream = Quitar flujo
ui-jobs-mix = Mezcla
ui-mix-weighted = Ponderada
ui-mix-sequential = Secuencial
ui-mix-round-robin = Round robin
ui-jobs-target = Dónde
ui-jobs-backend = Backend
ui-jobs-iface = Interfaz
ui-jobs-choose-iface = Elija una interfaz…
ui-jobs-iface-not-needed = No es necesaria para este backend
ui-jobs-iface-denied = no permitida
ui-jobs-dry-run = Simulación: nombre del archivo pcapng
ui-jobs-dry-run-hint = Si se indica, no se envía nada: los paquetes se escriben en este archivo dentro del directorio de capturas del servidor.
ui-jobs-how = Cómo
ui-jobs-rate = Tasa
ui-jobs-count = Número de paquetes
ui-jobs-duration = Duración
ui-jobs-burst = Ráfaga
ui-jobs-workers = Workers
ui-jobs-workers-disabled = Varios workers requieren un backend multi-worker y un único flujo.
ui-jobs-from-template = Desde la plantilla
ui-jobs-name = Nombre
ui-jobs-name-placeholder = De forma predeterminada, el nombre de la plantilla
ui-jobs-start-now = Iniciar de inmediato
ui-jobs-create = Crear trabajo
ui-jobs-create-start = Crear e iniciar
ui-jobs-created = Trabajo “{ $id }” creado.
ui-jobs-need-iface = Elija una interfaz.
ui-jobs-need-template = Elija una plantilla para cada flujo.
ui-jobs-bad-weight = Los pesos deben ser enteros positivos.
ui-rate-default = Vacío: la tasa de la plantilla (o el límite del servidor).
ui-rate-is-pps = → { $rate }
ui-rate-is-bps = → { $rate } en el medio (capa 1)
ui-rate-is-percent = → { $rate } % de la velocidad del enlace
ui-rate-is-ifg = Intervalo fijo entre tramas: { $rate }
ui-rate-is-unlimited = Lo más rápido posible
ui-valid-empty = Obligatorio.
ui-valid-number = No es un número.
ui-valid-integer = Introduzca un número entero.
ui-valid-range = Debe ser mayor que cero.
ui-valid-percent = Un porcentaje debe ser como máximo 100.
ui-valid-rate-unit = Unidad desconocida: use pps, kpps, Mpps, bps, kbps, Mbps, Gbps, %, ifg=<tiempo> o unlimited.
ui-valid-duration-unit = Unidad desconocida: use ns, us, ms, s, m o h.
ui-valid-capture-name = Use un nombre de archivo relativo sin “..”.
ui-confirm-send-title = ¿Enviar por una interfaz real?
ui-confirm-send-body = Este trabajo transmite paquetes en la red. Asegúrese de que la interfaz y la tasa son adecuadas para esta red.
ui-confirm-send = Enviar
ui-job-detail = Trabajo
ui-job-state = Estado
ui-job-start = Iniciar
ui-job-pause = Pausar
ui-job-resume = Reanudar
ui-job-stop = Detener
ui-job-download-report = Informe (JSON)
ui-job-live = Estadísticas en vivo
ui-job-streams = Por flujo
ui-job-inline-template = (plantilla en línea)
ui-job-workers = Por worker
ui-job-workers-pending = Los contadores por worker forman parte del informe final, disponible cuando el trabajo haya terminado.
ui-job-active = Tiempo activo
ui-job-core = Núcleo
ui-job-accuracy = Precisión del ritmo
ui-acc-target = Objetivo
ui-acc-achieved = Logrado
ui-acc-jitter = Jitter (medio / máx.)
ui-job-spec = Configuración
ui-job-created = Creado
ui-job-started = Iniciado
ui-job-finished = Terminado
ui-job-deleted = Trabajo eliminado.
ui-job-delete-title = ¿Eliminar este trabajo?
ui-job-delete-body = Si el trabajo está en ejecución, primero se detiene; su informe permanece en el historial.
ui-tpl-import = Importar
ui-tpl-new = Nueva plantilla
ui-tpl-search = Buscar nombre, descripción, etiqueta, protocolo…
ui-tpl-source = Origen
ui-tpl-source-all = Todas
ui-tpl-source-library = Biblioteca
ui-tpl-source-user = Mías
ui-tpl-favorites-only = Solo favoritas
ui-tpl-tags = Etiquetas
ui-tpl-recent = Usadas recientemente
ui-tpl-empty = Ninguna plantilla coincide.
ui-tpl-favorite = Favorita
ui-tpl-view = Ver / editar YAML
ui-tpl-open-builder = Abrir en el editor
ui-tpl-new-job = Nuevo trabajo con esta plantilla
ui-tpl-export = Descargar YAML
ui-tpl-duplicate = Duplicar
ui-tpl-imported = Plantilla “{ $id }” importada.
ui-tpl-duplicated = Copia “{ $id }” creada.
ui-tpl-deleted = Plantilla “{ $id }” eliminada.
ui-tpl-delete-title = ¿Eliminar esta plantilla?
ui-tpl-delete-body = “{ $id }” se eliminará de su directorio de plantillas.
ui-tpl-saved = Plantilla guardada.
ui-tpl-yaml = YAML de la plantilla
ui-tpl-read-only = Las plantillas de la biblioteca son de solo lectura: duplíquela para editar una copia.
ui-lint-run = Comprobar (lint)
ui-lint-clean = No se encontraron problemas.
ui-lint-summary = { $errors } errores, { $warnings } advertencias
ui-set-appearance = Apariencia
ui-set-account = Cuenta
ui-set-logged-in = Sesión iniciada como { $id }.
ui-set-auth-disabled = La autenticación no está habilitada en este servidor.
ui-set-client = Este navegador
ui-set-client-note = Se guarda solo en este navegador. La configuración del servidor (wpg.toml) no se expone a través de la API.
ui-set-api = API
ui-set-ws = Eventos
ui-set-ui-version = Versión de la interfaz
ui-set-api-docs = Documentación de la API (Swagger UI)
ui-set-about = Acerca de
ui-set-server-version = Versión del servidor
ui-set-api-version = Versión de la API
ui-set-platform = Plataforma
ui-set-features = Funciones opcionales
ui-set-locales = Idiomas
ui-set-license = wpg tiene doble licencia: MIT o Apache-2.0.
ui-builder-placeholder = El editor de paquetes (paleta de protocolos, pila de capas, editor de campos, vista hexadecimal / de árbol) llegará en el siguiente paso del hito.
ui-builder-placeholder-template = El editor de paquetes llegará en el siguiente paso del hito. La plantilla “{ $id }” se abrirá aquí.
ui-reason-end-of-input = Fin del archivo de captura
ui-valid-stream-id = Introduzca un número entero de 0 a 4294967295.
ui-jobs-verification = Verificación en recepción
ui-jobs-signature = Estampar una firma de verificación
ui-jobs-signature-hint = Escribe un id de flujo, un número de secuencia y la hora de TX en los últimos 24 octetos de carga útil de cada trama, para que una captura con “Verificar firmas” pueda medir la pérdida, el orden y la latencia.
ui-jobs-stream-id = Id de flujo de la firma (primer flujo)
ui-job-signature = Firma
ui-job-signature-on = Activada (id de flujo { $id } en adelante)
ui-rx-new = Nueva captura
ui-rx-file = Archivo de captura
ui-rx-file-hint = Nombre de un archivo pcap / pcapng en el directorio de capturas del servidor, p. ej., la salida de simulación de un trabajo.
ui-rx-iface-any = any (todas las interfaces)
ui-rx-filter = Filtro
ui-rx-filter-empty = Vacío: todas las tramas coinciden.
ui-rx-filter-checking = Comprobando el filtro…
ui-rx-filter-ok = Válido: { $filter }
ui-rx-filter-error = { $error } (en el carácter { $offset })
ui-rx-count-hint = Detener tras este número de tramas coincidentes.
ui-rx-keep = Tramas que conservar
ui-rx-keep-hint = Se conservan para mostrarlas las N primeras tramas coincidentes (como máximo 10000).
ui-rx-keep-max = Como máximo 10000.
ui-rx-save = Guardar como pcapng (nombre de archivo)
ui-rx-save-hint = Opcional: las tramas coincidentes también se escriben en este archivo en el directorio de capturas del servidor.
ui-rx-save-same = El archivo de salida debe ser distinto del de entrada.
ui-rx-name-placeholder = De forma predeterminada, backend:interfaz
ui-rx-options = Opciones
ui-rx-verify = Verificar firmas (pérdida, orden, latencia)
ui-rx-promisc = Modo promiscuo
ui-rx-outgoing = Incluir las tramas que envía este equipo
ui-rx-start = Iniciar captura
ui-rx-started = Captura “{ $id }” iniciada.
ui-rx-list = Capturas
ui-rx-empty = Aún no hay capturas.
ui-rx-source = Interfaz / archivo
ui-rx-actions = Acciones
ui-rx-stop = Detener
ui-rx-stop-named = Detener “{ $id }”
ui-rx-delete-named = Eliminar “{ $id }”
ui-rx-deleted = Captura eliminada.
ui-rx-delete-title = ¿Eliminar esta captura?
ui-rx-delete-body = Si la captura está en ejecución, primero se detiene. El archivo pcapng guardado permanece en el servidor.
ui-rx-download = Descargar pcapng
ui-rx-counters = Contadores
ui-rx-matched = Coincidentes
ui-rx-matched-pps = Paquetes coincidentes/s
ui-rx-matched-bps = Tasa de bits coincidente
ui-rx-matched-bytes = Bytes coincidentes
ui-rx-saved = Guardadas en archivo
ui-rx-kept = Conservadas para mostrar
ui-rx-verify-title = Verificación
ui-rx-verify-off = La verificación de firmas está desactivada para esta captura.
ui-rx-verify-none = Aún no se ha recibido ninguna trama firmada.
ui-rx-signed = Tramas firmadas
ui-rx-unsigned = Tramas sin firma
ui-rx-lost = Perdidas
ui-rx-stream = Flujo
ui-rx-received = Recibidas
ui-rx-seq-range = Números de secuencia
ui-rx-out-of-order = Fuera de orden
ui-rx-duplicates = Duplicadas
ui-rx-latency = Latencia (mín. / media / máx.)
ui-rx-jitter = Jitter
ui-rx-latency-note = Las tramas perdidas se estiman a partir de los huecos en los números de secuencia. La latencia solo tiene sentido si el emisor y el receptor comparten reloj (mismo equipo o sincronizados por PTP / NTP).
ui-rx-frames = Tramas capturadas
ui-rx-frames-empty = Aún no hay tramas coincidentes.
ui-rx-time = Tiempo (s)
ui-rx-length = Longitud
ui-rx-signature = Firma
ui-rx-sig-value = flujo { $stream } · sec { $seq }
ui-rx-show-more = Mostrar más ({ $count } de { $max })
ui-rx-frame-title = Trama { $index }
ui-rx-select-frame = Seleccione una trama para ver sus octetos y capas decodificadas.
ui-rx-decode-failed = No se puede decodificar la trama: { $error }
ui-rx-truncated = Solo se capturaron { $bytes } de { $len } octetos.

# --- Descripciones de campos de protocolos L2/encapsulación P1 ---
lldp-tlvs = TLV de LLDP (cabecera de tipo de 7 bits + longitud de 9 bits).
cdp-version = Versión del protocolo CDP (2).
cdp-ttl = Tiempo durante el que este anuncio es válido, en segundos.
cdp-checksum = Suma de comprobación de Internet sobre la cabecera CDP y los TLV (Auto).
cdp-tlvs = TLV de CDP (tipo de 16 bits, longitud de 16 bits que incluye la cabecera).
stp-protocol-id = Identificador del protocolo Spanning Tree (siempre 0).
stp-version = Versión del protocolo (0 = STP, 2 = RSTP, 3 = MSTP).
stp-type = Tipo de BPDU (0 = configuración, 2 = RST/MST, 0x80 = TCN).
stp-flags = Indicadores de la BPDU (cambio de topología, propuesta, rol, aprendizaje, reenvío, acuerdo, TCA).
stp-root-priority = Prioridad del puente raíz y extensión del system-id.
stp-root-mac = Dirección MAC del puente raíz.
stp-root-path-cost = Coste de la ruta hasta el puente raíz.
stp-bridge-priority = Prioridad del puente emisor y extensión del system-id.
stp-bridge-mac = Dirección MAC del puente emisor.
stp-port-id = Identificador del puerto emisor (prioridad + número).
stp-message-age = Antigüedad de la BPDU, en 1/256 de segundo.
stp-max-age = Antigüedad máxima de la BPDU antes de expirar, en 1/256 de segundo.
stp-hello-time = Intervalo entre BPDU de configuración, en 1/256 de segundo.
stp-forward-delay = Temporizador de retardo de reenvío, en 1/256 de segundo.
stp-trailer = Extensión RSTP / MSTP (longitud version-1, longitud v3, registros MSTI); octetos opacos.
lacp-subtype = Subtipo de Slow Protocols (1 = LACP).
lacp-actor-system = Dirección MAC del sistema actor.
lacp-actor-state = Indicadores de estado del actor (actividad, timeout, agregación, sincronización, recopilación, distribución, predeterminado, caducado).
lacp-partner-system = Dirección MAC del sistema socio.
lacp-partner-state = Indicadores de estado del socio.
marker-subtype = Subtipo de Slow Protocols (2 = Marker).
eapol-version = Versión del protocolo EAPOL.
eapol-type = Tipo de paquete EAPOL (0 = EAP, 1 = Start, 2 = Logoff, 3 = Key).
eapol-length = Longitud del cuerpo EAPOL, en octetos (Auto).
eapol-body = Cuerpo EAPOL (el paquete EAP para el tipo 0); octetos opacos.
pppoe-version = Versión del protocolo PPPoE (1).
pppoe-type = Tipo PPPoE (1).
pppoe-code = Código PPPoE (PADI / PADO / PADR / PADS / PADT, 0 = datos de sesión).
pppoe-session-id = Identificador de sesión PPPoE.
pppoe-length = Longitud de la carga útil PPPoE, en octetos (Auto).
pppoe-tags = TAG de descubrimiento PPPoE (tipo de 16 bits, longitud de 16 bits).
ppp-protocol = Identificador de protocolo PPP de la carga útil (LCP / IPCP / IP / IPv6 / ...).
ppp-cp-code = Código del protocolo de control PPP (Configure-Request / Ack / Nak / ...).
ppp-cp-identifier = Identificador de solicitud del protocolo de control PPP.
ppp-cp-length = Longitud del paquete del protocolo de control PPP, en octetos (Auto).
ppp-cp-options = Opciones del protocolo de control PPP (TLV de tipo / longitud de 8 bits).
vxlan-flags = Indicadores VXLAN (bit I = el VNI es válido).
vxlan-vni = VXLAN Network Identifier de 24 bits.
gre-flags = Indicadores y versión de GRE (bits de presencia de suma de comprobación / clave / secuencia).
gre-protocol-type = EtherType de la carga útil encapsulada.
nvgre-vsid = Virtual Subnet ID de NVGRE de 24 bits.
nvgre-flowid = FlowID de NVGRE (entropía para el balanceo de carga).
mpls-label = Valor de etiqueta MPLS de 20 bits. 0-15 están reservados (0 / 2 explicit null, 1 router alert, 3 implicit null, 7 entropy label indicator, 13 GAL, 14 OAM alert, 15 extensión); los SID de SR-MPLS son base SRGB + índice de SID.
mpls-tc = Clase de tráfico de 3 bits (QoS / ECN).
mpls-s = Bit de fondo de pila (1 = última etiqueta; Auto según la capa siguiente).
mpls-ttl = Tiempo de vida (TTL) de MPLS.
gtpu-version = Versión de GTP (1).
gtpu-pt = Tipo de protocolo (1 = GTP, 0 = GTP').
gtpu-e = Indicador de presencia de cabecera de extensión.
gtpu-s = Indicador de presencia de número de secuencia.
gtpu-pn = Indicador de presencia de número N-PDU.
gtpu-message-type = Tipo de mensaje GTP-U (255 = G-PDU).
gtpu-length = Longitud de la carga útil tras la cabecera obligatoria, en octetos (Auto).
gtpu-teid = Tunnel Endpoint Identifier.
geneve-version = Versión de Geneve (0).
geneve-opt-len = Longitud de la región de opciones, en palabras de 4 octetos.
geneve-o = Indicador de paquete OAM.
geneve-c = Indicador de presencia de opciones críticas.
geneve-protocol-type = EtherType de la carga útil encapsulada.
geneve-vni = Virtual Network Identifier de Geneve de 24 bits.
geneve-options = Opciones de Geneve (opacas; tamaño según opt_len).
l2tp-flags = Indicadores y versión de L2TP (bits de tipo, longitud, secuencia, desplazamiento, prioridad).
l2tp-body = Cuerpo L2TP (campos condicionales según presencia + carga útil); octetos opacos.
l2tpv3-session-id = Session ID de L2TPv3.

# --- Editor de paquetes (M4 fase 2) ---
ui-bld-palette = Protocolos
ui-bld-search = Buscar protocolos
ui-bld-favorite = Favorito
ui-bld-favorites = Favoritos
ui-bld-layer-l2 = Enlace de datos (L2)
ui-bld-layer-l3 = Red (L3)
ui-bld-layer-l4 = Transporte (L4)
ui-bld-layer-l7 = Aplicación (L7)
ui-bld-layer-payload = Carga útil
ui-bld-stack = Capas
ui-bld-empty = Arrastre aquí un protocolo o añádalo desde la paleta.
ui-bld-unknown = desconocido
ui-bld-dup = Duplicar capa
ui-bld-suggest = Añadir capa siguiente
ui-bld-binding-warn = No anunciada por la capa inferior
ui-bld-fix = Corregir
ui-bld-added = Se añadió { $id }.
ui-bld-moved = Movida a la posición { $index }.
ui-bld-removed = Capa eliminada.
ui-bld-fields = Editor de campos
ui-bld-select-layer = Seleccione una capa para editar sus campos.
ui-bld-auto = Auto
ui-bld-manual = Manual
ui-bld-gen = Gen
ui-bld-computed = se calcula al construir
ui-bld-nsap-parts = AFI { $afi } · área { $area } · system ID { $sys } · NSEL { $nsel }
ui-bld-estimate = Paquetes del flujo y su tamaño total
ui-bld-expands = se expande a { $count } (~{ $bytes })
ui-bld-rate = Tasa
ui-bld-rate-hint = Tasa de transmisión guardada con la plantilla (1000pps, 1.5Mpps, 10Gbps, ifg=10us)
ui-bld-duration = ~{ $time } de envío
ui-bld-duration-unknown = tiempo de envío: defina una tasa
ui-bld-duration-hint = Paquetes que enviar (tx.count o, si no, la longitud del flujo) a esta tasa, limitado por tx.duration_secs
ui-bld-gen-kind = Tipo
ui-bld-gen-count = recuento:
ui-bld-gen-shorthand = Rango / lista / CIDR
ui-bld-gen-random = Aleatorio
ui-bld-gen-random-mac = MAC aleatoria
ui-bld-gen-counter = Contador
ui-bld-gen-file = Archivo
ui-bld-gen-expr = Expresión
ui-bld-hex = Hex
ui-bld-tree = Árbol
ui-bld-no-bytes = Aún no hay paquete.
ui-bld-name = Nombre de la plantilla
ui-bld-need-name = Primero asigne un nombre a la plantilla.
ui-bld-unsaved = Cambios sin guardar
ui-bld-save-as = Guardar como…
ui-bld-save-first = Guarde la plantilla antes de crear un trabajo.
ui-bld-recompute = Recalcular
ui-bld-auto-recompute = Recalcular la vista previa en vivo con cada edición
ui-bld-index = Índice en el flujo
ui-bld-index-prev = Paquete anterior
ui-bld-index-next = Paquete siguiente
ui-bld-undo = Deshacer
ui-bld-redo = Rehacer
ui-bld-parity-ok = La vista previa del navegador coincide con el renderizado del servidor.
ui-bld-parity-ok-short = coincide
ui-bld-parity-bad = La vista previa del navegador no coincide con el renderizado del servidor.
ui-bld-parity-bad-short = no coincide
ui-bld-parity-pending = Comprobando el renderizado del servidor…
ui-bld-help-title = Atajos del editor
ui-bld-help-add = Añadir el protocolo enfocado
ui-bld-help-move = Mover la capa seleccionada
ui-bld-help-del = Quitar la capa seleccionada
ui-bld-help-undo = Deshacer
ui-bld-help-redo = Rehacer
ui-bld-help-hex = Escribir hex sobre el byte del cursor
ui-bld-help-caret = Mover el cursor hex
ui-bld-suggest-prev = Añadir capa encima

# --- Descripciones de campos de protocolos de redundancia / multidifusión P1 ---
vrrp-version = Versión de VRRP (2 = RFC 3768, 3 = RFC 5798).
vrrp-type = Tipo de paquete VRRP (1 = Advertisement).
vrrp-vrid = Identificador de router virtual (VRID).
vrrp-priority = Prioridad del router emisor (255 = propietario de la dirección, 0 = deja de participar).
vrrp-count = Número de direcciones virtuales en el anuncio (Auto).
vrrp-auth-type = Tipo de autenticación de VRRPv2 (0 = ninguna, 1 = texto simple, 2 = IP AH).
vrrp-adver-int = Intervalo de anuncio de VRRPv2, en segundos.
vrrp-max-adver-int = Intervalo máximo de anuncio de VRRPv3, en centésimas de segundo.
vrrp-checksum = Suma de comprobación de Internet sobre el mensaje VRRP (VRRPv3 incluye la pseudocabecera IP) (Auto).
vrrp-addresses = Direcciones del router virtual.
vrrp-auth-data = Datos de autenticación de VRRPv2 (8 octetos).
hsrp-version = Versión de HSRP (0 para HSRPv1).
hsrp-op-code = Operación HSRP (0 = Hello, 1 = Coup, 2 = Resign, 3 = Advertise).
hsrp-state = Estado del emisor (1 = Learn, 2 = Listen, 4 = Speak, 8 = Standby, 16 = Active).
hsrp-hello-time = Intervalo entre mensajes Hello, en segundos.
hsrp-hold-time = Tiempo durante el que un Hello es válido, en segundos.
hsrp-priority = Prioridad del router dentro del grupo.
hsrp-group = Número de grupo standby.
hsrp-auth-data = Contraseña de autenticación en texto claro (8 octetos, predeterminada "cisco").
hsrp-virtual-ip = Dirección IPv4 virtual del grupo.
hsrpv2-tlvs = TLV de HSRPv2 (Group State, Interface State, autenticación de texto / MD5; tipo y longitud de 8 bits).
glbp-version = Versión de GLBP (1).
glbp-group = Número de grupo GLBP.
glbp-owner-id = Dirección MAC del router emisor.
glbp-tlvs = TLV de GLBP (Hello, Request/Response, autenticación; la longitud incluye la cabecera).
igmp-type = Tipo de mensaje IGMP (Auto según el cuerpo: 0x11 consulta, 0x12 / 0x16 informe, 0x17 abandono, 0x22 informe v3).
igmp-max-resp-code = Tiempo / código de respuesta máximo en 1/10 s (consultas; cero en los informes).
igmp-checksum = Suma de comprobación de Internet sobre todo el mensaje IGMP (Auto).
igmp-group = Dirección del grupo de multidifusión (0.0.0.0 en una consulta general).
igmp-s-flag = Indicador de supresión del procesamiento en el router.
igmp-qrv = Variable de robustez del consultante (QRV).
igmp-qqic = Código de intervalo de consulta del consultante (QQIC).
igmp-num-sources = Número de direcciones de origen.
igmp-sources = Direcciones de origen.
igmp-num-records = Número de registros de grupo.
igmp-records = Registros de grupo (tipo de registro, longitud de datos auxiliares, orígenes, dirección de multidifusión).
mld-max-resp = Retardo / código de respuesta máximo, en milisegundos.
mld-address = Dirección de multidifusión IPv6 (:: en una consulta general).
pim-version = Versión de PIM (2).
pim-type = Tipo de mensaje PIM (Auto según el cuerpo: 0 Hello, 1 Register, 3 Join/Prune).
pim-checksum = Suma de comprobación de Internet (todo el mensaje; Register: solo los 8 primeros octetos) (Auto).
pim-hello-options = Opciones de PIM Hello (Holdtime, LAN Prune Delay, DR Priority, Generation ID, Address List).
pim-addr-family = Familia de direcciones de la dirección codificada (1 = IPv4).
pim-upstream-neighbor = Dirección del vecino upstream al que se dirige el Join/Prune.
pim-num-groups = Número de conjuntos de grupos.
pim-holdtime = Tiempo durante el que se mantiene el estado Join/Prune, en segundos.
pim-groups = Conjuntos de grupos (grupo codificado, orígenes codificados unidos y podados).
pim-register-border = Bit Border (el emisor es un PIM Multicast Border Router).
pim-register-null = Bit Null-Register (sin paquete de datos encapsulado).

# --- Descripciones de campos de protocolos de aplicación P1 ---
dhcp-op = Código de operación del mensaje BOOTP (1 = BOOTREQUEST, 2 = BOOTREPLY).
dhcp-htype = Tipo de dirección de hardware (1 = Ethernet).
dhcp-hlen = Longitud de la dirección de hardware, en octetos (6 para Ethernet).
dhcp-hops = Número de saltos de retransmisión; cada agente de retransmisión lo incrementa.
dhcp-xid = ID de transacción elegido por el cliente.
dhcp-secs = Segundos transcurridos desde que el cliente inició la adquisición.
dhcp-flags = Indicadores BOOTP (bit 15 = se solicita respuesta por difusión).
dhcp-ciaddr = Dirección IP del cliente (cuando ya está vinculada).
dhcp-yiaddr = Dirección IP "your" (del cliente) asignada por el servidor.
dhcp-siaddr = Dirección IP del siguiente servidor (servidor de arranque).
dhcp-giaddr = Dirección IP del agente de retransmisión (gateway); distinta de cero cuando se retransmite.
dhcp-chaddr = Dirección de hardware (MAC) del cliente; seguida de 10 octetos de relleno.
dhcp-sname = Nombre de host del servidor opcional (64 octetos, rellenado con NUL).
dhcp-file = Nombre del archivo de arranque (128 octetos, rellenado con NUL).
dhcp-magic-cookie = Magic cookie de DHCP (0x63825363).
dhcp-options = Opciones DHCP (código / longitud de 8 bits; Pad, End, Message Type, Relay Agent Information, ...).
dhcpv6-msg-type = Tipo de mensaje DHCPv6 (Solicit, Advertise, Request, Reply, Relay-Forward, ...).
dhcpv6-transaction-id = ID de transacción de 24 bits.
dhcpv6-options = Opciones DHCPv6 (código / longitud de 16 bits; Client ID, IA_NA, Relay Message, ...).
dhcpv6-hop-count = Número de agentes de retransmisión que han retransmitido este mensaje.
dhcpv6-link-address = Dirección que identifica el enlace del cliente (retransmisión).
dhcpv6-peer-address = Dirección del cliente o del agente de retransmisión del que procede el mensaje.
dns-id = Identificador de la consulta, copiado en la respuesta.
dns-qr = Consulta (0) o respuesta (1).
dns-opcode = Tipo de consulta (0 = consulta estándar, 4 = NOTIFY, 5 = UPDATE).
dns-aa = Indicador de respuesta autoritativa.
dns-tc = Indicador de truncamiento.
dns-rd = Indicador de recursión deseada.
dns-ra = Indicador de recursión disponible.
dns-z = Bit reservado (debe ser 0).
dns-ad = Indicador de datos auténticos (DNSSEC).
dns-cd = Indicador de comprobación desactivada (DNSSEC).
dns-rcode = Código de respuesta (0 = NOERROR, 3 = NXDOMAIN, ...).
dns-qdcount = Número de entradas en la sección de preguntas.
dns-ancount = Número de registros de recursos en la sección de respuestas.
dns-nscount = Número de registros de recursos en la sección de autoridad.
dns-arcount = Número de registros de recursos en la sección adicional.
dns-sections = Secciones de pregunta / respuesta / autoridad / adicional (nombres con etiquetas prefijadas por longitud); octetos opacos.
ntp-li = Indicador de segundo intercalar (3 = reloj no sincronizado).
ntp-version = Número de versión de NTP (4).
ntp-mode = Modo de asociación (3 = cliente, 4 = servidor, 5 = difusión).
ntp-stratum = Estrato (0 = sin especificar, 1 = referencia primaria, 2-15 = secundario).
ntp-poll = Intervalo de sondeo máximo, en log2 segundos (con signo).
ntp-precision = Precisión del reloj, en log2 segundos (con signo).
ntp-root-delay = Retardo de ida y vuelta hasta el reloj de referencia (segundos en coma fija 16.16).
ntp-root-dispersion = Dispersión respecto al reloj de referencia (segundos en coma fija 16.16).
ntp-reference-id = ID de referencia (código ASCII para el estrato 1; si no, una dirección IPv4 o un hash).
ntp-reference-ts = Momento en que se ajustó por última vez el reloj del sistema (marca de tiempo NTP de 64 bits).
ntp-origin-ts = Hora de transmisión del cliente devuelta por el servidor (marca de tiempo NTP de 64 bits).
ntp-receive-ts = Momento en que la solicitud llegó al servidor (marca de tiempo NTP de 64 bits).
ntp-transmit-ts = Momento en que el paquete salió del emisor (marca de tiempo NTP de 64 bits).
ntp-extensions = Campos de extensión y MAC; octetos opacos.
snmp-tag = Octeto de etiqueta BER (0x30 SEQUENCE, 0x02 INTEGER, 0x04 OCTET STRING).
snmp-msg-length = Longitud BER de la SEQUENCE del mensaje SNMP (Auto; forma corta, menor que 128).
snmp-version-length = Longitud BER del INTEGER de versión (1).
snmp-version = Versión de SNMP (0 = v1, 1 = v2c).
snmp-community-length = Longitud BER de la cadena de comunidad, en octetos.
snmp-community = Cadena de comunidad (p. ej., "public").
snmp-pdu-type = Etiqueta de la PDU (0xa0 GetRequest, 0xa2 Response, 0xa4 Trap v1, 0xa7 SNMPv2-Trap, ...).
snmp-pdu-length = Longitud BER del contenido de la PDU (Auto; forma corta, menor que 128).
snmp-pdu-body = Contenido de la PDU como TLV BER: request-id, error-status, error-index y la SEQUENCE variable-bindings.
snmp-trap-body = Contenido del trap como TLV BER: enterprise, agent-addr, generic-trap, specific-trap, time-stamp y variable bindings.
syslog-message = Texto del mensaje syslog, que empieza por <PRI> (facility * 8 + severity).
radius-code = Tipo de paquete RADIUS (Access-Request, Access-Accept, Accounting-Request, ...).
radius-identifier = Identificador que empareja solicitudes y respuestas.
radius-length = Longitud de todo el paquete RADIUS, en octetos (Auto).
radius-authenticator = Request / Response Authenticator de 16 octetos (Manual).
radius-attributes = Atributos RADIUS (tipo de 8 bits, longitud de 8 bits que incluye la cabecera).
tftp-opcode = Código de operación TFTP (1 = RRQ, 2 = WRQ, 3 = DATA, 4 = ACK, 5 = ERROR, 6 = OACK).
tftp-filename = Nombre de archivo (terminado en NUL).
tftp-mode = Modo de transferencia: netascii, octet o mail (terminado en NUL).
tftp-options = Pares de cadenas nombre / valor de opción (RFC 2347): blksize, timeout, tsize, windowsize, ...
tftp-block = Número de bloque (empieza en 1).
tftp-data = Octetos de datos (512 por bloque; un bloque más corto finaliza la transferencia).
tftp-error-code = Código de error TFTP (1 = archivo no encontrado, 2 = infracción de acceso, ...).
tftp-error-message = Mensaje de error (terminado en NUL).

# --- Descripciones de campos de protocolos de enrutamiento P1 ---
ospf-version = Versión de OSPF (2 = OSPFv2, 3 = OSPFv3).
ospf-type = Tipo de paquete OSPF (Hello / DB Description / LS Request / LS Update / LS Ack; Auto según la capa siguiente).
ospf-packet-length = Longitud del paquete OSPF, cabecera incluida, en octetos (Auto).
ospf-router-id = Router ID del router emisor.
ospf-area-id = Área a la que pertenece el paquete (0.0.0.0 = backbone).
ospf-checksum = Suma de comprobación de Internet sobre el paquete OSPF sin el campo de autenticación (Auto).
ospf-au-type = Tipo de autenticación (0 = nula, 1 = contraseña simple, 2 = criptográfica).
ospf-authentication = Campo de autenticación de 64 bits (contraseña, o key id / secuencia para la autenticación criptográfica).
ospf-network-mask = Máscara de red de la interfaz o de la red anunciada.
ospf-hello-interval = Intervalo entre paquetes Hello, en segundos.
ospf-options = Capacidades opcionales (bits E, MC, N/P, L, DC, O, DN).
ospf-router-priority = Prioridad del router para la elección del DR (0 = nunca DR).
ospf-router-dead-interval = Segundos sin Hello antes de declarar caído al vecino.
ospf-designated-router = Router designado (0.0.0.0 = ninguno).
ospf-backup-designated-router = Router designado de respaldo (0.0.0.0 = ninguno).
ospf-neighbors = Router ID de los vecinos vistos en el enlace.
ospf-interface-mtu = Datagrama IP más grande que la interfaz puede enviar sin fragmentar.
ospf-dbd-flags = Indicadores de Database Description (I = init, M = more, MS = master).
ospf-dd-sequence = Número de secuencia de Database Description.
ospf-lsa-headers = Cabeceras LSA (20 octetos cada una; sumas de comprobación y longitudes en Manual).
ospf-lsr-requests = LSA solicitados (LS type, Link State ID, router anunciante).
ospf-num-lsas = Número de LSA en la actualización.
ospf-ls-age = Tiempo desde que se originó el LSA, en segundos (mantenga 0 para una suma de comprobación Auto exacta).
ospf-ls-type = Tipo de LSA (Auto según la capa del cuerpo del LSA; 9 / 10 / 11 = LSA opaco).
ospf-link-state-id = Link State ID que identifica la parte de la red descrita.
ospf-advertising-router = Router ID del router que originó el LSA.
ospf-ls-sequence = Número de secuencia del LSA (0x80000001 = inicial).
ospf-ls-checksum = Suma de comprobación Fletcher del LSA sin el LS age (Auto).
ospf-lsa-length = Longitud del LSA, cabecera incluida, en octetos (Auto).
ospf-router-lsa-flags = Indicadores de Router-LSA (V = extremo de enlace virtual, E = ASBR, B = ABR).
ospf-num-links = Número de enlaces de router descritos.
ospf-router-links = Enlaces de router (link ID, link data, tipo, número de TOS, métrica).
ospf-attached-routers = Router ID de los routers conectados a la red.
ospf-metric = Coste de la ruta.
ospf-external-e = Tipo de métrica externa (1 = tipo 2, 0 = tipo 1).
ospf-forwarding-address = Dirección a la que se reenvía el tráfico hacia el destino (0.0.0.0 = el ASBR).
ospf-external-route-tag = Etiqueta de 32 bits asociada a la ruta externa.
ospf-opaque-ls-type = Ámbito de inundación del LSA opaco (9 = enlace local, 10 = área, 11 = AS).
ospf-opaque-type = Tipo opaco (1 = TE, 4 = Router Information, 7 = Extended Prefix, 8 = Extended Link; Auto según la capa del cuerpo).
ospf-opaque-id = Opaque ID que distingue los LSA del mismo tipo opaco (24 bits).
ospf-opaque-tlvs = TLV del LSA opaco (tipo y longitud de 16 bits, valores rellenados a 4 octetos; longitudes en Auto).
ospfv3-checksum = Suma de comprobación de Internet sobre el paquete OSPFv3 con la pseudocabecera IPv6 (Auto).
ospfv3-instance-id = Instance ID (separa instancias OSPFv3 en un mismo enlace).
ospfv3-interface-id = Interface ID de la interfaz emisora.
ospfv3-options = Opciones de OSPFv3 (bits V6, E, N, R, DC, AF).
ospfv3-ls-type = Código de función del LSA con los bits de ámbito U / S (Auto según la capa del cuerpo del LSA).
ospfv3-router-links = Interfaces del router (tipo, métrica, interface ID, router ID del vecino).
ospfv3-prefix-length = Longitud del prefijo en bits.
ospfv3-prefix-options = Opciones del prefijo (bits NU, LA, P, DN).
ospfv3-address-prefix = Prefijo de dirección, rellenado hasta un múltiplo de 4 octetos.
ospfv3-external-flags = Indicadores de AS-External-LSA (E = métrica de tipo 2, F = hay dirección de reenvío, T = hay etiqueta de ruta).
ospfv3-referenced-ls-type = LS type del LSA referenciado.
ospfv3-link-local-address = Dirección de enlace local de la interfaz.
ospfv3-num-prefixes = Número de prefijos que siguen.
ospfv3-prefixes = Entradas de prefijo (longitud, opciones, métrica / reservado, prefijo de dirección); octetos opacos.
ospfv3-referenced-link-state-id = Link State ID del LSA referenciado.
ospfv3-referenced-advertising-router = Router anunciante del LSA referenciado.
bgp-marker = Marcador de 16 octetos (todo unos).
bgp-length = Longitud del mensaje BGP, cabecera incluida, en octetos (Auto).
bgp-type = Tipo de mensaje BGP (OPEN / UPDATE / NOTIFICATION / KEEPALIVE / ROUTE-REFRESH; Auto según la capa siguiente).
bgp-version = Versión de BGP (4).
bgp-my-as = Número de sistema autónomo del emisor (23456 = AS_TRANS para AS de 4 octetos).
bgp-hold-time = Hold time propuesto, en segundos (0 o al menos 3).
bgp-identifier = Identificador BGP (router ID) del emisor.
bgp-opt-params-length = Longitud de los parámetros opcionales, en octetos (Auto).
bgp-opt-params = Parámetros opcionales (tipo 2 = capacidades).
bgp-withdrawn-routes-length = Longitud de las rutas retiradas, en octetos (Auto).
bgp-withdrawn-routes = Rutas IPv4 retiradas (longitud del prefijo en bits, prefijo).
bgp-total-path-attribute-length = Longitud de los atributos de ruta, en octetos (Auto).
bgp-path-attributes = Atributos de ruta (indicadores + código de tipo; la longitud es de 16 bits con el indicador Extended Length, que se activa automáticamente por encima de 255 octetos).
bgp-nlri = Rutas IPv4 anunciadas (longitud del prefijo en bits, prefijo); las demás familias van en MP_REACH_NLRI.
bgp-error-code = Código de error de NOTIFICATION.
bgp-error-subcode = Subcódigo de error de NOTIFICATION.
bgp-error-data = Datos de diagnóstico de NOTIFICATION.
bgp-afi = Address Family Identifier (1 = IPv4, 2 = IPv6).
bgp-rr-subtype = Subtipo de ROUTE-REFRESH (0 = normal, 1 = BoRR, 2 = EoRR).
bgp-safi = Subsequent Address Family Identifier (1 = unicast, 2 = multicast).
rip-command = Comando RIP (1 = solicitud, 2 = respuesta).
rip-version = Versión de RIP (1 o 2; RIPng usa 1).
rip-entries = Entradas de ruta (AFI, etiqueta de ruta, dirección, máscara, siguiente salto, métrica).
ripng-entries = Entradas de la tabla de rutas (prefijo, etiqueta de ruta, longitud del prefijo, métrica; 0xff = siguiente salto).
eigrp-version = Versión de la cabecera EIGRP (2).
eigrp-opcode = Código de operación EIGRP (Update / Query / Reply / Hello / SIA-Query / SIA-Reply).
eigrp-checksum = Suma de comprobación de Internet sobre el paquete EIGRP (Auto).
eigrp-flags = Indicadores EIGRP (Init, Conditional Receive, Restart, End of Table).
eigrp-sequence = Número de secuencia para la entrega fiable.
eigrp-acknowledge = Número de secuencia confirmado (0 = ninguno).
eigrp-virtual-router-id = ID de router virtual (0 = familia de direcciones unicast).
eigrp-autonomous-system = Número de sistema autónomo EIGRP.
eigrp-tlvs = TLV de EIGRP (tipo de 16 bits, longitud de 16 bits que incluye la cabecera).

# --- Descripciones de campos de encapsulación OSI P2 ---
osi-nlpid = Identificador de protocolo de capa de red (ISO/IEC TR 9577): 0x81 CLNP, 0x82 ES-IS, 0x83 IS-IS.
osi-data = Resto de la PDU OSI tras el NLPID (octetos opacos).
clnp-nlpid = Identificador de protocolo de capa de red (0x81 = CLNP).
clnp-length-indicator = Longitud de la cabecera en octetos, hasta el final de las opciones (Auto).
clnp-version = Versión / extensión del ID de protocolo (1).
clnp-lifetime = Tiempo de vida restante en unidades de 500 ms.
clnp-sp = SP: segmentación permitida; la parte de segmentación está presente (Auto según la parte de segmentación).
clnp-ms = MS: siguen más segmentos.
clnp-er = E/R: enviar un informe de error si la PDU se descarta.
clnp-pdu-type = Tipo de PDU: 0x1c DT (datos), 0x1d MD (datos de multidifusión), 0x01 ER (informe de error), 0x1e ERQ / 0x1f ERP (solicitud / respuesta de eco).
clnp-segment-length = Longitud de toda la PDU, cabecera y datos (Auto).
clnp-checksum = Suma de comprobación Fletcher ISO 8473 de la cabecera (Auto). 0 = sin suma de comprobación.
clnp-destination = Dirección NSAP de destino.
clnp-source = Dirección NSAP de origen.
clnp-segmentation = Parte de segmentación (identificador de unidad de datos, desplazamiento del segmento, longitud total), presente cuando SP está activado. La longitud total no se calcula.
clnp-options = Opciones (código de 8 bits / longitud de 8 bits) hasta el indicador de longitud: seguridad, mantenimiento de QoS, prioridad, enrutamiento de origen, registro de ruta, relleno, motivo del descarte.
esis-nlpid = Identificador de protocolo de capa de red (0x82 = ES-IS).
esis-length-indicator = Longitud de la PDU en octetos (Auto).
esis-version = Versión / extensión del ID de protocolo (1).
esis-pdu-type = Tipo de PDU: 2 ESH (end system hello), 4 ISH (intermediate system hello), 6 RD (redirección).
esis-holding-time = Tiempo de retención en segundos.
esis-checksum = Suma de comprobación Fletcher ISO 8473 de la PDU (Auto). 0 = sin suma de comprobación.
esis-number-of-source-addresses = Número de direcciones de origen (Auto).
esis-source-addresses = Direcciones NSAP atendidas por el sistema final.
esis-net = Network entity title del sistema intermedio (redirección: vacío cuando el siguiente salto es un sistema final).
esis-destination = Dirección NSAP de destino que se redirige.
esis-bsnpa-length = Longitud de la dirección de subred (Auto).
esis-bsnpa = Dirección de subred (p. ej., dirección MAC) del mejor siguiente salto.
esis-options = Opciones (código de 8 bits / longitud de 8 bits) hasta el indicador de longitud: seguridad, mantenimiento de QoS, prioridad, máscara de dirección, máscara SNPA, temporizador de configuración de ES sugerido.
esis-data = Resto de la PDU tras la parte fija (octetos opacos).
isis-irpd = Discriminador de protocolo de enrutamiento intradominio (0x83 = IS-IS).
isis-header-length = Longitud de la cabecera fija en octetos (constante para cada tipo de PDU: LAN Hello / LSP 27, P2P Hello 20, CSNP 33, PSNP 17).
isis-version-ext = Versión / extensión del ID de protocolo (1).
isis-id-length = Longitud del system ID (0 = 6 octetos).
isis-pdu-type = Tipo de PDU (15/16 LAN Hello L1/L2, 17 P2P Hello, 18/20 LSP L1/L2, 24/25 CSNP, 26/27 PSNP).
isis-version = Versión (1).
isis-max-area-addresses = Número máximo de direcciones de área (0 = 3).
isis-pdu-length = Longitud de toda la PDU (Auto). En un Hello, un valor Manual rellena la PDU hasta esa longitud con TLV de relleno (p. ej., 1497 para una MTU de 1500).
isis-tlvs = TLV (código de 8 bits / longitud de 8 bits) y sus sub-TLV.
isis-source-id = System ID del emisor.
isis-holding-time = Tiempo de retención en segundos.
isis-priority = Prioridad para la elección del IS designado (0–127).
isis-lan-id = LAN ID: system ID del IS designado e ID de pseudonodo.
isis-circuit-type = Tipo de circuito: 1 = L1, 2 = L2, 3 = L1/L2.
isis-local-circuit-id = ID de circuito local.
isis-remaining-lifetime = Tiempo de vida restante del LSP en segundos (no cubierto por la suma de comprobación).
isis-lsp-id = LSP ID: system ID, ID de pseudonodo y número de fragmento.
isis-sequence-number = Número de secuencia del LSP.
isis-lsp-checksum = Suma de comprobación Fletcher desde el LSP ID hasta el final de la PDU (Auto).
isis-partition-repair = Bit P: se admite la reparación de particiones.
isis-attached = Bits ATT: conectado a otra área (métrica default / delay / expense / error).
isis-overload = Bit OL: sobrecarga de la base de datos de LSP.
isis-is-type = Tipo de IS: 1 = L1, 3 = L1/L2.
isis-snp-source-id = System ID del emisor e ID de circuito (0).
isis-start-lsp-id = Primer LSP ID cubierto por la CSNP.
isis-end-lsp-id = Último LSP ID cubierto por la CSNP.
isis-data = Resto de la PDU tras la cabecera común (octetos opacos).

# --- Descripciones de campos de SCTP P2 ---
sctp-src-port = Puerto de origen.
sctp-dst-port = Puerto de destino.
sctp-verification-tag = Etiqueta de verificación: la etiqueta de inicio del par (0 en un paquete que lleva INIT).
sctp-checksum = CRC32c de todo el paquete SCTP con este campo a cero (Auto), enviado empezando por el octeto menos significativo.
sctp-chunks = Chunks (tipo de 8 bits, indicadores de 8 bits, longitud de 16 bits que incluye la cabecera de 4 octetos, relleno con ceros hasta 4 octetos): DATA, INIT, INIT ACK, SACK, HEARTBEAT, ABORT, SHUTDOWN, ERROR, COOKIE ECHO / ACK, AUTH, I-DATA, ASCONF, RE-CONFIG, FORWARD TSN, PAD. Las longitudes, el relleno y los recuentos de SACK son Auto.

# --- Descripciones de campos de LDP / RSVP-TE / BFD P2 ---
ldp-version = Versión de LDP (1).
ldp-pdu-length = Longitud de la PDU tras los campos de versión y de longitud de PDU (Auto).
ldp-lsr-id = LSR ID: los cuatro primeros octetos del identificador LDP.
ldp-label-space = ID del espacio de etiquetas: los dos últimos octetos del identificador LDP (0 = para toda la plataforma).
ldp-messages = Mensajes (tipo de 16 bits con el bit U, longitud de 16 bits del valor, ID de mensaje, TLV): Notification, Hello, Initialization, KeepAlive, Address / Address Withdraw, Label Mapping / Request / Withdraw / Release / Abort Request. TLV: FEC, Address List, Hop Count, Path Vector, Generic Label, Status, Common Hello / Session Parameters, direcciones de transporte, Configuration Sequence Number. Las longitudes son Auto.
rsvp-version = Versión de RSVP (1).
rsvp-flags = Indicadores (bit 0: admite reducción de refresco, RFC 2961).
rsvp-msg-type = Tipo de mensaje (1 Path, 2 Resv, 3 PathErr, 4 ResvErr, 5 PathTear, 6 ResvTear, 7 ResvConf, 12 Bundle, 13 Ack, 15 Srefresh, 20 Hello).
rsvp-checksum = Suma de comprobación de Internet de todo el mensaje con este campo a cero (Auto).
rsvp-send-ttl = TTL IP con el que se envió el mensaje.
rsvp-reserved = Reservado (0).
rsvp-length = Longitud de todo el mensaje en octetos, incluida la cabecera común (Auto).
rsvp-objects = Objetos (longitud de 16 bits que incluye la cabecera de 4 octetos, Class-Num, C-Type): SESSION, RSVP_HOP, TIME_VALUES, ERROR_SPEC, SCOPE, STYLE, FLOWSPEC, FILTER_SPEC, SENDER_TEMPLATE, SENDER_TSPEC, RESV_CONFIRM, LABEL, LABEL_REQUEST, EXPLICIT_ROUTE, RECORD_ROUTE, HELLO, MESSAGE_ID, SESSION_ATTRIBUTE. Las longitudes, la longitud del nombre de sesión y el relleno son Auto.
bfd-version = Versión de BFD (1).
bfd-diag = Código de diagnóstico: el motivo del último cambio de estado de la sesión local.
bfd-state = Estado de la sesión: 0 AdminDown, 1 Down, 2 Init, 3 Up.
bfd-p = Bit P: Poll (un cambio de parámetros; el par responde con F).
bfd-f = Bit F: Final (respuesta a un Poll).
bfd-c = Bit C: Control Plane Independent.
bfd-a = Bit A: autenticación presente (Auto: se activa cuando hay una sección de autenticación).
bfd-d = Bit D: modo Demand.
bfd-m = Bit M: Multipoint (0).
bfd-detect-mult = Multiplicador del tiempo de detección.
bfd-length = Longitud del paquete de control en octetos, incluida la sección de autenticación (Auto).
bfd-my-discriminator = My discriminator: un valor distinto de cero, único para la sesión en el emisor.
bfd-your-discriminator = Your discriminator: el discriminador del par (0 mientras se desconoce).
bfd-desired-min-tx = Intervalo mínimo de TX deseado en microsegundos.
bfd-required-min-rx = Intervalo mínimo de RX requerido en microsegundos.
bfd-required-min-echo-rx = Intervalo mínimo de RX de eco requerido en microsegundos (0 = sin eco).
bfd-authentication = Sección de autenticación (tipo de 8 bits, longitud de 8 bits que incluye la cabecera): contraseña simple, keyed MD5 / SHA1 y sus variantes meticulous. El digest / hash es Manual.

# --- Documentación generada (`wpg docs gen`, crates/wpg-cli/src/cmd/docs) ---
docs-gen-no-root = { $path } no es la raíz del repositorio (no existe docs/en/src/SUMMARY.md); indique --root.
docs-gen-error = Error al generar la documentación: { $error }
docs-gen-stale = desactualizado: { $path }
docs-gen-extra = ya no se genera: { $path }
docs-gen-up-to-date = La documentación generada está actualizada.
docs-gen-run-hint = Ejecute `just docs-gen` (o `wpg docs gen`) y confirme el resultado.
docs-gen-done = Documentación generada: { $written } archivos escritos, { $removed } eliminados.
docgen-yes = sí
docgen-octets = { $octets } octetos
docgen-items = { $count } elementos
docgen-variant = Variante { $n }
docgen-col-id = ID
docgen-col-name = Nombre
docgen-col-fields = Campos
docgen-col-next-by = Capa siguiente según
docgen-col-notes = Notas
docgen-col-lower = Capa inferior
docgen-col-selector = Selector
docgen-col-value = Valor
docgen-col-protocol = Protocolo
docgen-col-note = Nota
docgen-col-level = Nivel
docgen-col-rule = Regla
docgen-col-field = Campo
docgen-col-type = Tipo
docgen-col-default = Predeterminado
docgen-col-auto = Auto
docgen-col-description = Descripción
docgen-col-bit = Bit
docgen-col-mask = Máscara
docgen-col-type-value = Tipo
docgen-col-value-fields = Campos del valor
docgen-col-key = Clave
docgen-col-required = Obligatorio
docgen-col-in = En
docgen-col-status = Estado
docgen-col-body = Cuerpo
docgen-col-tag = Etiqueta
docgen-col-endpoint = Endpoint
docgen-col-summary = Resumen
docgen-col-env = Variable de entorno
docgen-layer-l2 = L2 (enlace de datos)
docgen-layer-l3 = L3 (red)
docgen-layer-l4 = L4 (transporte)
docgen-layer-l7 = Aplicación y control
docgen-layer-payload = Carga útil
docgen-proto-title = Referencia de protocolos
docgen-proto-intro = Este capítulo se genera a partir de la IR de definición de protocolos de `wpg-protocols` (el catálogo que sirve `GET /api/protocols`), por lo que siempre coincide con el código: { $count } protocolos. Cada página enumera los campos en el orden en que van en el medio, con su tipo, valor predeterminado, regla Auto y descripción; las capas inferiores que conducen al protocolo; la tabla de la capa siguiente; las restricciones que se comprueban tras la construcción y las plantillas de ejemplo incluidas. Las [notas de protocolos](../protocols/overview.md) escritas a mano explican cómo se llega a los protocolos, qué campos se definen a mano y las aproximaciones que quedan. Tras cambiar una definición, regenere con `just docs-gen`.
docgen-proto-layer = Capa
docgen-proto-header = Cabecera
docgen-proto-header-fixed = { $octets } octetos (fija)
docgen-proto-header-variable = longitud variable
docgen-proto-min-frame = Trama mínima
docgen-proto-synth = Generador
docgen-proto-synth-payload = generador de carga útil integrado (los campos siguientes son parámetros, no campos del paquete)
docgen-proto-pseudo = Pseudocabecera
docgen-proto-pseudo-fields = proporciona { $src } / { $dst } a las sumas de comprobación de capas superiores
docgen-proto-notes = Notas
docgen-proto-final-dst = Destino final
docgen-proto-final-dst-text = mientras { $when } no sea 0, el elemento 0 de { $list } sustituye la dirección de destino de la pseudocabecera inferior (RFC 8200 §8.1)
docgen-proto-inbound = Se llega desde
docgen-proto-inbound-none = Ningún protocolo del catálogo conduce aquí: úselo como primera capa o apílelo explícitamente.
docgen-proto-fields = Campos
docgen-proto-params = Parámetros
docgen-proto-no-fields = Sin campos.
docgen-proto-next = Capa siguiente
docgen-proto-next-intro = La capa siguiente se selecciona mediante { $field }; cuando el campo está en Auto, el constructor escribe el valor del protocolo apilado encima.
docgen-proto-next-alt = Al decodificar, también se consulta { $field } si el primer campo no coincide con ninguna entrada.
docgen-proto-constraints = Restricciones
docgen-proto-samples = Plantillas de ejemplo
docgen-proto-samples-none = Ninguna plantilla incluida usa todavía este protocolo.
docgen-proto-samples-more = y { $count } más
docgen-bind-default = (cualquier otro valor)
docgen-bind-min-len = quedan al menos { $octets } octetos
docgen-bind-reparse = vuelve a decodificar esta capa
docgen-bind-peek = Primeros { $bits } bits de la carga útil cuando { $field } = { $value }
docgen-bind-peek-skip = { $bits } bits de la carga útil tras sus primeros { $skip } bits cuando { $field } = { $value }
docgen-level-error = error
docgen-level-warning = advertencia
docgen-level-info = información
docgen-c-range = { $field } dentro de { $min }..{ $max }
docgen-c-equals = { $field } = { $value }
docgen-c-not-equals = { $field } ≠ { $value }
docgen-c-min-length = { $span } de al menos { $octets } octetos
docgen-c-max-length = { $span } de como máximo { $octets } octetos
docgen-span-header = cabecera
docgen-span-payload = carga útil
docgen-span-total = cabecera y carga útil
docgen-unit-bytes = octetos
docgen-unit-words16 = palabras de 16 bits
docgen-unit-words32 = palabras de 32 bits
docgen-unit-words64 = palabras de 64 bits
docgen-auto-length = Longitud de { $span } en { $unit }
docgen-auto-checksum = { $algorithm } sobre { $covers }
docgen-auto-nonzero = un 0 calculado se envía como todo unos
docgen-auto-exclude = { $fields } se cuentan como cero
docgen-auto-next = Número de protocolo de la capa siguiente (véase Capa siguiente)
docgen-auto-count = Número de elementos de { $field }
docgen-auto-size = Tamaño de { $field } en octetos
docgen-auto-counter = Contador por flujo (por ahora se usa el valor predeterminado)
docgen-cksum-internet = Suma de comprobación de Internet
docgen-cksum-internet-pseudo = Suma de comprobación de Internet con pseudocabecera
docgen-cksum-fletcher = Suma de comprobación Fletcher
docgen-cksum-crc32c = CRC-32C
docgen-cksum-crc32 = CRC-32
docgen-cover-header = la cabecera
docgen-cover-all = la cabecera y todo lo que le sigue
docgen-len-from-field = { $expr } octetos
docgen-len-count = { $field } elementos
docgen-len-until = hasta el desplazamiento { $expr } del grupo
docgen-len-to-end = resto de la PDU
docgen-len-to-end-except = resto de la PDU salvo sus últimos { $octets } octetos
docgen-len-until-terminator = hasta el terminador { $octets } (se añade al construir)
docgen-len-until-tlv = hasta el primer elemento de tipo { $type } inclusive, o el final de la PDU
docgen-tlv-uncounted = Los primeros { $octets } octetos del valor tras la longitud no se cuentan en ella.
docgen-list-item = Cada elemento es un grupo de los campos siguientes.
docgen-tlv-format = Tipo de { $type } bits, longitud de { $len } bits.
docgen-tlv-len-first = La longitud va antes del tipo.
docgen-tlv-len-incl = La longitud cuenta también los campos de tipo y longitud.
docgen-tlv-len-excl = La longitud cuenta solo el valor.
docgen-tlv-unit = Unidad de longitud: { $unit }.
docgen-tlv-head = Los primeros { $octets } octetos del valor se sitúan entre el tipo y la longitud.
docgen-tlv-align = Cada elemento se rellena con ceros hasta un múltiplo de { $align } octetos (no contado en la longitud)
docgen-tlv-unpadded-last = el último elemento no se rellena
docgen-tlv-long = El bit de tipo { $flag } selecciona una longitud de { $len } bits.
docgen-tlv-fill = Un valor Manual de { $field } rellena la región con elementos { $variant } hasta ese tamaño.
docgen-tlv-unknown-raw = Los tipos desconocidos se conservan como octetos de valor en bruto.
docgen-tlv-unknown-stop = Un tipo desconocido pone fin a la decodificación tipada (el resto pasa a ser un único elemento en bruto).
docgen-tlv-type-only = (solo tipo, sin longitud ni valor)
docgen-tlv-when-prefix = cuando el valor empieza por { $octets }
docgen-tlv-when-len = cuando el valor mide { $octets } octetos
docgen-notes-heading = Referencia generada
docgen-notes-intro = Referencia de campos de los protocolos de esta página (generada a partir de las definiciones):
docgen-cli-title = Referencia de la CLI
docgen-cli-intro = El binario `wpg` ejecuta tanto el servidor (`wpg serve`) como las herramientas sin interfaz gráfica, por lo que también sirve para CI y sesiones ssh. Por ejemplo, `wpg send tpl.wpg.yaml -i eth0 --rate 1Mpps` transmite una plantilla. Esta página se genera a partir de las definiciones de los comandos: cada bloque es la salida literal de `--help` de la compilación actual. Los mensajes en tiempo de ejecución siguen `LANG` / `LC_ALL`; el propio texto de `--help` está en inglés.
docgen-cli-commands = Comandos
docgen-cli-note-send = Envía una plantilla. `--plan` elige entre expandir todas las tramas de antemano (`prerender`) y construirlas mientras se envían (`onthefly`). `--json` imprime el informe final en stdout.
docgen-cli-note-render = Escribe las tramas de una plantilla en un archivo pcapng sin transmitir.
docgen-cli-note-import = Convierte una captura (o hex pegado) en una plantilla; consulte el [tutorial de reproducción de pcap](../tutorials/pcap-replay.md).
docgen-cli-note-bench = Transmisión a tasa ilimitada de una trama sintética (EtherType 0x88b5), o de una plantilla con `--template`. Consulte `docs/perf.md` para ver las referencias. Campos de la salida `--json`: backend, detail, iface, frame_size, batch_size, packets, dropped, pps, bps (L2), l1_bps, elapsed.
docgen-cli-note-ifaces = Lista las interfaces con MAC, direcciones, MTU, estado del enlace y controlador.
docgen-cli-note-doctor = Ejecuta las comprobaciones del entorno descritas en [Permisos](../getting-started/permissions.md); el código de salida es distinto de cero cuando falla una comprobación.
docgen-cli-note-template = Herramientas de plantillas. `lint` sale con el código 1 si hay errores; `export` e `import` empaquetan plantillas en archivos `.wpgbundle` y las desempaquetan de ellos.
docgen-cli-note-config = Herramientas de `wpg.toml`; consulte la [referencia de configuración](config.md).
docgen-api-title = Referencia de la API
docgen-api-intro = wpg-server ofrece una API REST (JSON) bajo `/api` y envía estadísticas en vivo, cambios de estado de los trabajos y registros a través de un WebSocket (`GET /api/ws`). Esta página se genera a partir del documento OpenAPI del servidor (utoipa). Un servidor en ejecución sirve el mismo documento en `/api/openapi.json` con Swagger UI en `/api/docs/`, y se mantiene una copia en [`docs/schema/openapi.json`](https://github.com/SeiyaFunaokaJP/web-packet-generator/blob/main/docs/schema/openapi.json). Inicie sesión con `POST /api/auth/login` (o `POST /api/auth/setup` en la primera ejecución); a partir de entonces la cookie `wpg_session` autentica cada solicitud, y las solicitudes POST / PUT / DELETE deben enviar además el token CSRF de `GET /api/auth/status` en la cabecera `X-CSRF-Token`. Los errores son un objeto JSON `error` con `code`, `message` (en el idioma de `Accept-Language`) y `details`. Las descripciones de endpoints y esquemas siguientes proceden del código.
docgen-api-endpoints = Endpoints
docgen-api-params = Parámetros
docgen-api-body = Cuerpo de la solicitud
docgen-api-responses = Respuestas
docgen-api-schemas = Esquemas
docgen-api-schemas-intro = Cuerpos de solicitud y de respuesta referenciados arriba.
docgen-api-tag-auth = Inicio y cierre de sesión y configuración inicial.
docgen-api-tag-system = Estado, versión, interfaces, autodiagnóstico y backends de transmisión.
docgen-api-tag-protocols = El catálogo de protocolos (consulte la [referencia de protocolos](protocols.md)).
docgen-api-tag-packets = Renderizado, decodificación y lint de paquetes.
docgen-api-tag-templates = La biblioteca de plantillas.
docgen-api-tag-jobs = Trabajos de transmisión y estadísticas.
docgen-api-tag-events = El flujo de eventos WebSocket.
docgen-config-title = Configuración
docgen-config-intro = El servidor lee `wpg.toml` del directorio de trabajo, o el archivo indicado con `--config` / `WPG_CONFIG`. Precedencia, de menor a mayor: valores predeterminados integrados, el archivo, las variables de entorno `WPG_<SECTION>__<KEY>` (`__` separa los niveles de anidamiento, por ejemplo `WPG_SERVER__BIND=0.0.0.0:8080` o `WPG_TX__ALLOWED_IFACES='["eth1"]'`) y, por último, las opciones de línea de comandos como `wpg serve --bind`. Las claves desconocidas son errores. `wpg config init` escribe el ejemplo comentado siguiente, `wpg config show` muestra la configuración efectiva, `wpg config validate` la comprueba y `wpg config set-password` guarda el hash de la contraseña de administrador. La contraseña de administrador en sí no es una clave: `wpg serve --password`, `--password-file` y `WPG_PASSWORD` tienen prioridad sobre `auth.password_hash` y `auth.password_file`. Esta página se genera a partir de los tipos de configuración.
docgen-config-keys = Claves
docgen-config-top-level = Nivel superior
docgen-config-default-machine = Directorio de datos del sistema operativo (Linux: `~/.local/share/wpg`)
docgen-config-example = Ejemplo (`wpg config init`)
docgen-config-example-intro = Cada línea activa es igual al valor predeterminado integrado, por lo que el archivo no cambia nada hasta que se edita; las líneas comentadas muestran las claves opcionales.
docgen-config-data-dir = Directorio para el estado del servidor: plantillas de usuario, favoritos, la base de datos SQLite (usuarios, sesiones, historial de trabajos), capturas de simulación y el certificado TLS autofirmado.
docgen-config-server = Escucha HTTP.
docgen-config-server-bind = Dirección de escucha. Sin contraseña (y sin ningún usuario creado en la pantalla de configuración inicial) solo se aceptan direcciones de loopback (`--allow-insecure` lo anula).
docgen-config-server-tls = Cadena de certificados TLS y clave privada (PEM) con los que servir HTTPS; tiene prioridad sobre `self_signed_tls`.
docgen-config-server-tls-cert = Cadena de certificados PEM.
docgen-config-server-tls-key = Clave privada PEM.
docgen-config-server-self-signed-tls = Servir HTTPS con un certificado autofirmado generado en `<data_dir>/tls/` (reutilizado en los siguientes arranques); `wpg serve --tls` lo activa.
docgen-config-server-base-url = URL pública del servidor detrás de un proxy inverso (`https://wpg.example.net`), usada para los enlaces absolutos.
docgen-config-auth = Autenticación.
docgen-config-auth-password-hash = Cadena PHC argon2id de la contraseña de administrador; `wpg config set-password` la escribe.
docgen-config-auth-password-file = Archivo que contiene la contraseña de administrador: una cadena PHC argon2 o texto sin formato que se convierte en hash al arrancar.
docgen-config-auth-session-ttl-secs = Duración de la sesión en segundos (deslizante: cada solicitud la prolonga).
docgen-config-templates = Biblioteca de plantillas.
docgen-config-templates-library-paths = Bibliotecas de plantillas de solo lectura (por ejemplo, checkouts de Git), en las que se buscan recursivamente `*.wpg.yaml` / `*.wpg.json` y que se recargan cuando cambian los archivos.
docgen-config-templates-user-dir = Directorio con permiso de escritura para las plantillas creadas o importadas en la GUI; predeterminado `<data_dir>/templates`.
docgen-config-protocols = Protocolos definidos por el usuario.
docgen-config-protocols-dirs = Directorios de archivos de protocolos definidos por el usuario (`*.yaml`: nuevas definiciones de protocolo y enlaces bajo protocolos existentes), cargados al iniciar en orden de nombre de archivo. `wpg serve --protocols-dir` añade más.
docgen-config-tx = Política y valores predeterminados de transmisión.
docgen-config-tx-allowed-ifaces = Interfaces por las que pueden enviar los trabajos; vacío = todas. El backend `null` y las simulaciones no tienen restricciones.
docgen-config-tx-max-rate = Tasa máxima que puede solicitar un trabajo (`100kpps`, `1Gbps`); los trabajos sin tasa se envían a esta tasa.
docgen-config-tx-default-backend = Backend que se usa cuando un trabajo no indica ninguno (`null`, `pcapfile`, `afpacket`, `pcap`, `afxdp`, `dpdk`); sin definir = `afpacket` en Linux; si no, `pcap`; si no, `null`.
docgen-config-tx-dpdk-eal-args = Argumentos EAL de DPDK para el backend `dpdk`, p. ej., `["-a", "0000:00:04.0"]`. wpg añade `-l <primera CPU>` y `--in-memory` si no se indican. EAL se inicia una vez por proceso con el primer trabajo `dpdk`; para cambiarlos hay que reiniciar.
docgen-config-log = Registro.
docgen-config-log-level = Directivas de filtro (`RUST_LOG` las anula), por ejemplo `info` o `wpg_tx=debug,info`.
docgen-config-log-format = Formato de línea: `pretty` (legible) o `json` (un objeto por línea).
docgen-config-log-file = Escribir también en este archivo (se crea su directorio).
docgen-config-log-rotation = Rotación del archivo: `never`, `hourly` o `daily`.
docgen-config-log-max-files = Conservar como máximo este número de archivos rotados (se eliminan los más antiguos).
docgen-config-log-ansi = Colorear la salida de la consola cuando es un terminal.
docgen-schema-heading = Referencia de claves (generada)
docgen-schema-intro = Generada a partir del JSON Schema del formato de plantilla (`wpg template schema`); las descripciones son los comentarios de documentación del código (en inglés). `Template` es la raíz del documento.
# --- Descripciones de campos de PTP / ESMC / CFM / R-APS / OAM 802.3ah P2 ---
ptp-major-sdo-id = majorSdoId (transportSpecific en IEEE 1588-2008): 0 para los perfiles predeterminados, 1 para IEEE 802.1AS (gPTP).
ptp-message-type = Tipo de mensaje: 0 Sync, 1 Delay_Req, 2 Pdelay_Req, 3 Pdelay_Resp, 8 Follow_Up, 9 Delay_Resp, 10 Pdelay_Resp_Follow_Up, 11 Announce, 12 Signaling, 13 Management.
ptp-minor-version = minorVersionPTP (0 en IEEE 1588-2008, 1 en IEEE 1588-2019).
ptp-version = versionPTP (2).
ptp-message-length = Longitud de todo el mensaje en octetos, incluidos los TLV (Auto).
ptp-domain-number = Número de dominio.
ptp-minor-sdo-id = minorSdoId (reservado, 0, en IEEE 1588-2008).
ptp-flags = Campo de indicadores: alternateMaster, twoStep, unicast y los bits específicos del perfil y de seguridad en el primer octeto; leap61, leap59, currentUtcOffsetValid, ptpTimescale, timeTraceable, frequencyTraceable y synchronizationUncertain en el segundo.
ptp-correction-field = Corrección en nanosegundos multiplicada por 2^16 (con signo).
ptp-message-type-specific = Campo específico del tipo de mensaje (reservado, 0, en IEEE 1588-2008).
ptp-clock-identity = Identidad de reloj del puerto de origen (EUI-64, p. ej., una dirección MAC con ff:fe en medio).
ptp-port-number = Número de puerto del puerto de origen.
ptp-sequence-id = ID de secuencia; un Follow_Up / Delay_Resp / Pdelay_Resp copia el del mensaje al que responde.
ptp-control-field = Campo de control (compatibilidad con IEEE 1588 versión 1): 0 Sync, 1 Delay_Req, 2 Follow_Up, 3 Delay_Resp, 4 Management, 5 otros mensajes.
ptp-log-message-interval = Log2 del intervalo entre mensajes en segundos (con signo; 0x7f si no procede).
ptp-timestamp-seconds = Marca de tiempo: segundos (48 bits).
ptp-timestamp-nanoseconds = Marca de tiempo: nanosegundos (32 bits).
ptp-port-identity-clock = Identidad de puerto: identidad de reloj (todo unos = todos los relojes).
ptp-port-identity-port = Identidad de puerto: número de puerto (0xffff = todos los puertos).
ptp-current-utc-offset = Desfase UTC actual en segundos (TAI - UTC).
ptp-grandmaster-priority = Prioridad del grandmaster (algoritmo del mejor reloj maestro; gana el menor).
ptp-grandmaster-clock-class = Clase de reloj del grandmaster (6 = sincronizado con una referencia primaria, 248 = predeterminada, 255 = solo esclavo).
ptp-grandmaster-clock-accuracy = Exactitud del reloj del grandmaster (0x20 = dentro de 25 ns ... 0xfe = desconocida).
ptp-grandmaster-variance = offsetScaledLogVariance del grandmaster.
ptp-grandmaster-identity = Identidad de reloj del grandmaster.
ptp-steps-removed = Número de relojes frontera entre el grandmaster y el emisor.
ptp-time-source = Fuente de tiempo del grandmaster: reloj atómico, GPS, radio terrestre, PTP, NTP, ajuste manual, otra, oscilador interno.
ptp-starting-boundary-hops = Saltos de frontera iniciales de un mensaje Management.
ptp-boundary-hops = Saltos de frontera restantes.
ptp-action = Acción de Management: GET, SET, RESPONSE, COMMAND, ACKNOWLEDGE.
ptp-tlvs = TLV (tipo de 16 bits, longitud de 16 bits del valor, Auto): Management, Management Error Status, Organization Extension, negociación unicast (Request / Grant / Cancel / Acknowledge Cancel), Path Trace, Alternate Time Offset Indicator. Terminan en la longitud del mensaje.
ptp-data = Resto del mensaje tras la cabecera común, como octetos (hasta la longitud del mensaje).
esmc-subtype = Subtipo de Slow Protocols (0x0a = específico de la organización).
esmc-itu-oui = OUI de ITU-T (00-19-a7).
esmc-itu-subtype = Subtipo de ITU-T (1 = ESMC).
esmc-version = Versión de ESMC (1).
esmc-event-flag = Indicador de evento: 1 para una PDU de evento enviada de inmediato tras un cambio de QL, 0 para la PDU de información enviada una vez por segundo.
esmc-tlvs = TLV (tipo de 8 bits, longitud de 16 bits que incluye la cabecera de 3 octetos, Auto): TLV QL con el código SSM, TLV QL extendido con el código SSM mejorado, identidad de reloj SyncE y recuentos de relojes en cascada. Los octetos que les siguen (relleno) se conservan como un único elemento en bruto.
cfm-md-level = Nivel de dominio de mantenimiento / MEG (0-7).
cfm-version = Versión de CFM (0; 1 para R-APS de G.8032v2).
cfm-opcode = OpCode: 1 CCM, 2 LBR, 3 LBM, 4 LTR, 5 LTM, 33 AIS, 35 LCK, 37 TST, 39 Linear APS, 40 R-APS, 41 MCC, 42 LMR, 43 LMM, 45 1DM, 46 DMR, 47 DMM, 52 CSF, 53 1SL, 54 SLR, 55 SLM ...
cfm-flags = Indicadores (específicos del OpCode).
cfm-first-tlv-offset = Desplazamiento del primer TLV desde el final de este campo (constante para cada OpCode).
cfm-rdi = RDI: indicación de defecto remoto.
cfm-period = Periodo de transmisión: 1 = 3.33 ms, 2 = 10 ms, 3 = 100 ms, 4 = 1 s, 5 = 10 s, 6 = 1 min, 7 = 10 min.
cfm-sequence-number = Número de secuencia.
cfm-mep-id = MEP ID (1-8191).
cfm-maid = MAID / MEG ID, 48 octetos: formato y nombre del MD, formato, longitud y nombre corto del MA, rellenado con ceros (Y.1731: sin nombre de MD, formato 32 basado en ICC, longitud 13).
cfm-frame-counter = Contador de tramas (medición de pérdidas).
cfm-transaction-id = Identificador de transacción de loopback / linktrace.
cfm-use-fdb-only = UseFDBonly: solo la base de datos de reenvío decide el puerto de salida.
cfm-fwd-yes = FwdYes: el LTM se retransmitió.
cfm-terminal-mep = TerminalMEP: la respuesta procede del MEP de destino.
cfm-ttl = TTL de linktrace.
cfm-relay-action = Acción de retransmisión: 1 RlyHit, 2 RlyFDB, 3 RlyMPDB.
cfm-original-mac = Dirección MAC del MEP que inició el linktrace.
cfm-target-mac = Dirección MAC de destino del linktrace.
cfm-csf-type = Tipo de fallo de señal del cliente: 0 LOS, 1 FDI / AIS, 2 RDI, 3 DCI.
cfm-timestamp = Marca de tiempo en formato IEEE 1588 (segundos y nanosegundos de 32 bits).
cfm-source-mep-id = MEP ID de origen.
cfm-responder-mep-id = MEP ID del respondedor (0 en un SLM).
cfm-test-id = Test ID de la medición sintética de pérdidas.
cfm-tlvs = TLV (tipo de 8 bits, longitud de 16 bits del valor, Auto): Sender ID, Port Status, Data, Interface Status, Reply Ingress / Egress, LTM / LTR Egress Identifier, Organization-Specific, Test, cerrados por el End TLV (tipo 0).
cfm-data = Resto de la PDU tras el desplazamiento del primer TLV, como octetos (campos específicos del OpCode y TLV).
raps-request-state = Solicitud / estado: 0000 NR, 0111 MS, 1011 SF, 1101 FS, 1110 Event.
raps-sub-code = Subcódigo (0000 = solicitud de vaciado con la solicitud Event).
raps-rb = RB: el RPL está bloqueado.
raps-dnf = DNF: no vaciar la base de datos de reenvío.
raps-bpr = BPR: referencia del puerto bloqueado (0 = enlace de anillo 0, 1 = enlace de anillo 1).
raps-node-id = Node ID: dirección MAC del nodo del anillo emisor.
efm-oam-subtype = Subtipo de Slow Protocols (3 = OAM).
efm-oam-flags = Indicadores: fallo de enlace, dying gasp, evento crítico, evaluación local / remota y estable.
efm-oam-code = Código de OAMPDU: 0x00 Information, 0x01 Event Notification, 0x02 Variable Request, 0x03 Variable Response, 0x04 Loopback Control, 0xfe Organization Specific.
efm-oam-information-tlvs = TLV de información (tipo de 8 bits, longitud de 8 bits que incluye la cabecera, Auto): Local / Remote Information, Organization Specific, cerrados por el End TLV (tipo 0).
efm-oam-sequence-number = Número de secuencia de la notificación de eventos.
efm-oam-event-tlvs = TLV de eventos de enlace (tipo de 8 bits, longitud de 8 bits que incluye la cabecera, Auto): errored symbol period, errored frame, errored frame period, errored frame seconds summary, Organization Specific, cerrados por el End TLV (tipo 0).
efm-oam-variable-descriptors = Descriptores de variables como octetos: rama (1 octeto) y hoja (2 octetos) cada uno, cerrados por una rama cero.
efm-oam-variable-containers = Contenedores de variables como octetos: rama, hoja, ancho y valor cada uno, cerrados por una rama cero.
efm-oam-loopback-command = Comando de loopback remoto: 1 habilitar, 2 deshabilitar.
efm-oam-oui = Identificador único de organización (OUI).
efm-oam-data = Resto de la OAMPDU, como octetos.

# --- Descripciones de campos de MACsec / GTPv2-C / Diameter / SIP / RTP P2 ---
macsec-v = Bit V de TCI: versión (0).
macsec-es = Bit ES de TCI: End Station (el SCI es la dirección MAC de origen con el puerto 1).
macsec-sc = Bit SC de TCI: hay un SCI explícito (Auto: se activa cuando se indica el SCI).
macsec-scb = Bit SCB de TCI: difusión de copia única de EPON.
macsec-e = Bit E de TCI: los datos seguros están cifrados.
macsec-c = Bit C de TCI: los datos de usuario se modificaron (siempre junto con E para los conjuntos de cifrado predeterminados).
macsec-an = Número de asociación (0-3): la asociación segura dentro del canal.
macsec-sl = Short length: la longitud de los datos seguros cuando es inferior a 48 octetos; si no, 0 (Manual).
macsec-pn = Número de paquete (32 bits; la mitad inferior con XPN).
macsec-sci = Secure Channel Identifier: identificador del sistema (dirección MAC) e identificador del puerto; presente cuando SC está activado.
macsec-secure-data = Datos seguros: los datos de usuario cifrados (o solo con integridad) como octetos opacos; no se cifra nada.
macsec-icv = Integrity Check Value (16 octetos), opaco: no se calcula.
gtpv2-version = Versión de GTP (2).
gtpv2-p = Indicador P: sigue un mensaje adjunto (piggybacked) (la siguiente capa GTPv2-C).
gtpv2-t = Indicador T: el campo TEID está presente (Auto: se activa cuando se indica un TEID).
gtpv2-mp = Indicador MP: la prioridad del mensaje está presente.
gtpv2-message-type = Tipo de mensaje (Echo, Create / Modify / Delete Session, Create / Update / Delete Bearer, ...).
gtpv2-length = Longitud del mensaje en octetos, sin los 4 primeros octetos (Auto).
gtpv2-teid = Tunnel Endpoint Identifier del receptor (ausente en los mensajes Echo; 0 en una solicitud inicial).
gtpv2-sequence = Número de secuencia (24 bits) que empareja una respuesta con su solicitud.
gtpv2-message-priority = Prioridad del mensaje (0 la más alta); se usa cuando MP está activado.
gtpv2-ies = Elementos de información (tipo de 8 bits, longitud de 16 bits de los datos, octeto spare / instance): IMSI, Cause, Recovery, APN, AMBR, EBI, IP Address, MEI, MSISDN, PAA, Bearer QoS, RAT Type, Serving Network, F-TEID, Bearer Context (agrupado), PDN Type, ... Las longitudes son Auto.
diameter-version = Versión de Diameter (1).
diameter-length = Longitud del mensaje en octetos, incluidos la cabecera y el relleno de los AVP (Auto).
diameter-r = Indicador R: solicitud (0 = respuesta).
diameter-p = Indicador P: apto para proxy.
diameter-e = Indicador E: la respuesta contiene un error de protocolo.
diameter-t = Indicador T: mensaje posiblemente retransmitido.
diameter-command-code = Código de comando (Capabilities-Exchange, Device-Watchdog, Credit-Control, ...).
diameter-application-id = Application-ID (0 = mensajes comunes de Diameter).
diameter-hop-by-hop = Identificador Hop-by-Hop que empareja una respuesta con su solicitud en una conexión.
diameter-end-to-end = Identificador End-to-End que detecta solicitudes duplicadas.
diameter-avps = AVP (código de 32 bits, indicadores V / M / P, longitud de 24 bits sin el relleno, Vendor-ID opcional, datos rellenados a 4 octetos): AVP del protocolo base y de Credit-Control, AVP agrupados de un nivel de profundidad. Las longitudes, el relleno y el indicador V son Auto.
sip-start-line = Línea inicial: una línea de solicitud (método, Request-URI, SIP/2.0) o una línea de estado (SIP/2.0, código, motivo). El CRLF se añade automáticamente.
sip-headers = Líneas de cabecera (Nombre: valor), una por elemento; los CRLF y la línea vacía que las sigue se añaden automáticamente. Content-Length es Manual.
sip-body = Cuerpo del mensaje (una oferta / respuesta SDP, ...).
rtp-version = Versión de RTP (2).
rtp-padding = Bit P: octetos de relleno al final de la carga útil (el último indica su número, Manual).
rtp-x = Bit X: una extensión de cabecera sigue a la lista CSRC (Auto).
rtp-cc = Número de CSRC (Auto).
rtp-marker = Bit de marcador (definido por el perfil, p. ej., el inicio de un talkspurt).
rtp-payload-type = Tipo de carga útil (0 PCMU, 8 PCMA, 9 G.722, 18 G.729, 96-127 dinámico).
rtp-sequence = Número de secuencia; se incrementa en uno por cada paquete RTP.
rtp-timestamp = Marca de tiempo: instante de muestreo del primer octeto de la carga útil.
rtp-ssrc = Identificador de la fuente de sincronización.
rtp-csrc = Identificadores de fuentes contribuyentes (hasta 15).
rtp-extension = Extensión de cabecera: 16 bits definidos por el perfil (0xBEDE: elementos de un byte de RFC 8285), la longitud en palabras de 32 bits (Auto) y las palabras de la extensión.
rtcp-version = Versión de RTCP (2).
rtcp-padding = Bit P: octetos de relleno al final del paquete (Manual).
rtcp-count = Campo de recuento / formato (5 bits) de un tipo de paquete sin tipado.
rtcp-report-count = Número de informes de recepción (Auto).
rtcp-source-count = Número de fuentes: chunks SDES o identificadores BYE (Auto).
rtcp-subtype = Subtipo definido por la aplicación.
rtcp-packet-type = Tipo de paquete (200 SR, 201 RR, 202 SDES, 203 BYE, 204 APP, 205 RTPFB, 206 PSFB, 207 XR).
rtcp-length = Longitud del paquete en palabras de 32 bits menos uno (Auto).
rtcp-body = Cuerpo del paquete tras la cabecera común, como octetos.
rtcp-ssrc = SSRC del emisor de este paquete.
rtcp-ntp-timestamp = Marca de tiempo NTP (palabra más significativa; el campo siguiente es la fracción).
rtcp-rtp-timestamp = Marca de tiempo RTP correspondiente a la marca de tiempo NTP.
rtcp-packet-count = Número de paquetes del emisor.
rtcp-octet-count = Número de octetos de carga útil del emisor.
rtcp-report-blocks = Bloques de informe de recepción: SSRC, fracción perdida, pérdida acumulada, número de secuencia más alto extendido, jitter, LSR, DLSR.
rtcp-extension = Octetos de extensión específicos del perfil hasta la longitud del paquete.
rtcp-chunks = Chunks SDES: SSRC / CSRC y sus elementos (tipo, longitud (Auto), texto), terminados por un octeto nulo y rellenados a 32 bits.
rtcp-ssrcs = Identificadores SSRC / CSRC que abandonan la sesión.
rtcp-reason = Motivo opcional del abandono: longitud (Auto), texto, relleno a 32 bits.
rtcp-app-name = Nombre (4 caracteres ASCII).
rtcp-app-data = Datos dependientes de la aplicación (múltiplo de 4 octetos).

## SNMP tool

# wpg-snmp / /api/snmp / the SNMP screen of wpg-web (ADR-0010).
api-error-snmp-disabled = La herramienta SNMP está desactivada en este servidor (snmp.enabled = false).
api-error-snmp-target = Destino SNMP no válido: { $error }
api-error-snmp-oid = Objeto / OID no válido: { $error }
api-error-snmp-value = Valor no válido: { $error }
api-error-snmp-usm = Configuración SNMPv3 no válida: { $error }
api-error-snmp-request = La solicitud SNMP falló: { $error }
api-error-snmp-report = El agente respondió con un Report: { $error } (compruebe el usuario, las frases de paso y el nivel de seguridad).
api-error-snmp-timeout = Sin respuesta de { $target } tras { $count } intento(s). Compruebe la dirección, el puerto, la comunidad / el usuario y los cortafuegos.
api-error-snmp-bind = No se puede escuchar en { $addr }: { $error }
api-error-snmp-bind-permission = No se puede escuchar en { $addr }: permiso denegado. Los puertos por debajo de 1024 (como 162) requieren root o CAP_NET_BIND_SERVICE (sudo setcap cap_net_bind_service=+ep /path/to/wpg); o use un puerto como 1162.
api-error-snmp-mib = Archivo MIB { $file }: { $error }
api-error-snmp-mib-bundled = { $what } viene incluido con wpg y no se puede eliminar (suba una versión más reciente para reemplazarlo).
config-issue-snmp-listen = dirección de escucha no válida (se espera dirección:puerto, p. ej. 0.0.0.0:162)
config-issue-snmp-privileged-port = los puertos por debajo de 1024 requieren root o CAP_NET_BIND_SERVICE
ui-nav-snmp = SNMP
ui-shortcut-snmp = Ir a la herramienta SNMP
ui-snmp-disabled = La herramienta SNMP está desactivada en este servidor (snmp.enabled = false en wpg.toml).
ui-snmp-tab-ops = Operaciones
ui-snmp-tab-traps = Traps
ui-snmp-tab-mibs = MIB
ui-snmp-pcap = Grabar pcap
ui-snmp-pcap-hint = Graba cada datagrama SNMP que este servidor envía o recibe (solicitudes, respuestas, traps)
ui-snmp-pcap-frames = { $count } tramas
ui-snmp-pcap-download = Descargar la grabación
ui-snmp-pcap-clear = Borrar la grabación
ui-snmp-target = Destino
ui-snmp-profile = Perfil
ui-snmp-profile-choose = (elija un perfil)
ui-snmp-profile-name = Nombre del perfil
ui-snmp-profile-save = Guardar perfil
ui-snmp-profile-save-hint = Guarda el destino en este navegador (sin comunidad ni frases de paso)
ui-snmp-profile-delete = Eliminar perfil
ui-snmp-profile-note = Los perfiles se guardan solo en este navegador; las comunidades y frases de paso nunca se almacenan (ni aquí ni en el servidor).
ui-snmp-host = Host
ui-snmp-port = Puerto
ui-snmp-version = Versión
ui-snmp-timeout = Tiempo de espera (ms)
ui-snmp-retries = Reintentos
ui-snmp-community = Comunidad
ui-snmp-user = Usuario
ui-snmp-level = Nivel de seguridad
ui-snmp-auth-protocol = Protocolo de autenticación
ui-snmp-auth-password = Frase de paso de autenticación
ui-snmp-priv-protocol = Protocolo de privacidad
ui-snmp-priv-password = Frase de paso de privacidad
ui-snmp-context = Nombre de contexto
ui-snmp-v3-note = SNMPv3: las frases de paso necesitan al menos 8 caracteres; el Engine ID del agente se descubre automáticamente.
ui-snmp-community-note = v1 / v2c envían la comunidad en texto plano.
ui-snmp-need-host = Introduzca el host.
ui-snmp-bad-port = El puerto debe ser 1–65535.
ui-snmp-need-user = Introduzca el usuario SNMPv3.
ui-snmp-short-password = Las frases de paso necesitan al menos 8 caracteres.
ui-snmp-bad-number = El tiempo de espera y los reintentos deben ser números enteros.
ui-snmp-need-oid = Introduzca al menos un OID o nombre de objeto.
ui-snmp-operations = Solicitud
ui-snmp-oids = OID / nombres de objetos
ui-snmp-oids-hint = Nombres de las MIB cargadas (ifDescr.1, IF-MIB::ifTable, sysUpTime.0) u OID con puntos, separados por espacios. Walk usa el primero. Intro ejecuta Get.
ui-snmp-op-get = Get
ui-snmp-op-getnext = GetNext
ui-snmp-op-getbulk = GetBulk
ui-snmp-op-walk = Walk
ui-snmp-op-set = Set
ui-snmp-max-repetitions = Max-repetitions
ui-snmp-walk-limit = Límite de walk
ui-snmp-auto-refresh = Actualización automática
ui-snmp-refresh-off = Desactivada
ui-snmp-refresh-every = Cada { $secs } s
ui-snmp-set-hint = El tipo «auto» toma el tipo de la MIB (se aceptan etiquetas de enumeración como down).
ui-snmp-oid = OID
ui-snmp-type = Tipo
ui-snmp-value = Valor
ui-snmp-value-placeholder = valor
ui-snmp-add-row = Añadir vínculo
ui-snmp-remove-row = Quitar
ui-snmp-type-auto = auto (de la MIB)
ui-snmp-type-integer = i: INTEGER
ui-snmp-type-unsigned = u: Unsigned32 / Gauge32
ui-snmp-type-counter32 = c: Counter32
ui-snmp-type-counter64 = C: Counter64
ui-snmp-type-timeticks = t: TimeTicks
ui-snmp-type-ipaddress = a: IpAddress
ui-snmp-type-oid = o: OBJECT IDENTIFIER
ui-snmp-type-string = s: cadena
ui-snmp-type-hex = x: octetos hex
ui-snmp-type-decimal = d: octetos decimales
ui-snmp-type-null = n: NULL
ui-snmp-type-opaque = Opaque (hex)
ui-snmp-result = Resultado
ui-snmp-raw = Valores sin procesar
ui-snmp-table-view = Vista de tabla
ui-snmp-no-result = Aún no se ha enviado ninguna solicitud.
ui-snmp-summary = { $action } { $target } ({ $version }): { $count } vínculos en { $time } ms, { $n } solicitud(es)
ui-snmp-error-status = El agente respondió { $error } (vínculo { $index }).
ui-snmp-truncated = El walk se detuvo en su límite; aumente «Límite de walk».
ui-snmp-engine-id = Engine ID { $id }
ui-snmp-name = Nombre
ui-snmp-rate = Tasa
ui-snmp-rate-history = Historial de tasa de { $target }
ui-snmp-index = Índice
ui-snmp-rows = { $count } filas
ui-snmp-receiver = Receptor de traps
ui-snmp-receiver-running = Escuchando en { $addr } · { $count } recibidos
ui-snmp-receiver-stopped = Detenido
ui-snmp-receiver-start = Iniciar
ui-snmp-receiver-restart = Reiniciar
ui-snmp-receiver-stop = Detener
ui-snmp-receiver-started = Receptor de traps escuchando en { $addr }
ui-snmp-listen = Dirección de escucha
ui-snmp-listen-hint = El puerto 162 requiere root o CAP_NET_BIND_SERVICE; si no, use p. ej. 0.0.0.0:1162 y apunte los equipos allí.
ui-snmp-accepted-communities = Comunidades aceptadas
ui-snmp-any-community = cualquiera (lista separada por comas para restringir)
ui-snmp-receiver-users = Usuarios SNMPv3 (traps e informs autenticados / cifrados)
ui-snmp-add-user = Añadir usuario
ui-snmp-receiver-engine = Engine ID de este receptor (para informs v3): { $id }
ui-snmp-received = Notificaciones recibidas
ui-snmp-filter = Filtrar
ui-snmp-clear-traps = Borrar
ui-snmp-no-traps = Aún no se ha recibido nada.
ui-snmp-time = Hora
ui-snmp-source = Origen
ui-snmp-notification = Notificación
ui-snmp-varbinds = Vínculos
ui-snmp-varbind-count = { $count } vínculos
ui-snmp-acknowledged = confirmado
ui-snmp-send = Enviar un trap / inform
ui-snmp-send-hint = v1 envía un Trap-PDU (convertido según RFC 3584), v2c / v3 un SNMPv2-Trap o un InformRequest (confirmado).
ui-snmp-send-to = Se envía con el destino de la pestaña Operaciones: { $target } ({ $version }).
ui-snmp-edit-target = Editar el destino
ui-snmp-trap-oid = OID del trap (snmpTrapOID)
ui-snmp-uptime = sysUpTime (1/100 s)
ui-snmp-uptime-auto = tiempo de actividad del servidor
ui-snmp-varbind-lines-hint = Un vínculo por línea: «objeto valor» (tipo de la MIB) u «objeto tipo valor», p. ej. ifOperStatus.2 down o sysName.0 s "core 1".
ui-snmp-bad-line = La línea { $index } necesita un objeto y un valor.
ui-snmp-inform = Inform (esperar la confirmación)
ui-snmp-send-button = Enviar
ui-snmp-trap-sent = Trap enviado a { $target }
ui-snmp-inform-acked = Inform confirmado por { $target }
ui-snmp-mib-upload = Subir MIB
ui-snmp-mib-drop = Suelte aquí los archivos MIB (.mib, .txt, .my), varios a la vez si se importan entre sí.
ui-snmp-mib-choose = Elegir archivos
ui-snmp-mib-upload-hint = Se guardan en el servidor en <data_dir>/mibs y se cargan al instante. Los IMPORTS que faltan se muestran; suba también esos módulos.
ui-snmp-mib-uploaded = { $count } archivo(s) MIB guardado(s)
ui-snmp-mib-no-module = no se encontró ningún módulo MIB (no guardado)
ui-snmp-mib-missing = { $what } importa módulos no cargados: { $list }
ui-snmp-mib-missing-short = faltan: { $list }
ui-snmp-mib-error-count = { $count } problema(s)
ui-snmp-mib-modules = Módulos MIB
ui-snmp-mib-show-bundled = Mostrar módulos incluidos
ui-snmp-mib-none = Aún no hay MIB subidas.
ui-snmp-mib-module = Módulo
ui-snmp-mib-source = Origen
ui-snmp-mib-status = Estado
ui-snmp-mib-problems = Problemas
ui-snmp-mib-bundled = incluido
ui-snmp-mib-nodes = { $count } objetos
ui-snmp-mib-ok = cargado
ui-snmp-mib-partial = cargado con problemas
ui-snmp-mib-error = no utilizable
ui-snmp-mib-delete-named = Eliminar { $what }
ui-snmp-mib-delete-title = ¿Eliminar el archivo MIB?
ui-snmp-mib-delete-body = Se elimina el archivo subido de este módulo, con todos los módulos que contiene.
ui-snmp-mib-deleted = { $what } eliminado
ui-snmp-mib-browser = Árbol MIB
ui-snmp-mib-search = Buscar objetos por nombre
ui-snmp-mib-node = Detalles del objeto
ui-snmp-mib-pick = Elija un objeto en el árbol o la búsqueda.
ui-snmp-mib-use = Usar en una solicitud
ui-snmp-mib-kind = Clase
ui-snmp-mib-syntax = Sintaxis
ui-snmp-mib-hint = Indicación de formato
ui-snmp-mib-access = Acceso
ui-snmp-mib-status-label = Estado
ui-snmp-mib-units = Unidades
ui-snmp-mib-index = Índice
ui-snmp-mib-augments = Augments
ui-snmp-mib-objects = Objetos
ui-snmp-mib-defval = Valor predeterminado
ui-snmp-mib-reference = Referencia
ui-snmp-mib-enums = Valores
ui-snmp-mib-description = Descripción

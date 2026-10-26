# Français (fr) — locale française. Chaque message doit exister dans tous les fichiers de locale
# (vérifié par les tests unitaires de wpg-i18n).

app-name = Web Packet Generator
app-tagline = Générateur de paquets haute performance pour les tests réseau, piloté depuis votre navigateur.

## Serveur / API
health-ok = Le service fonctionne correctement
server-listening = En écoute sur { $addr }
server-placeholder-ui = L'interface web n'a pas encore été compilée. Exécutez `trunk build` dans crates/wpg-web.

## Erreurs
error-not-implemented = `{ $command }` n'est pas encore implémenté.
error-not-found = Introuvable
error-internal = Erreur interne du serveur

## Interface
ui-language = Langue
ui-protocols = Protocoles

## Diagnostics (wpg-core `diag::keys` ; les noms d'arguments font partie du contrat)
diag-field-out-of-range = Le champ `{ $field }` = { $value } est hors de la plage { $min }..{ $max }.
diag-field-not-equal = Le champ `{ $field }` = { $value } diffère de la valeur attendue { $expected }.
diag-field-forbidden-value = Le champ `{ $field }` a une valeur déconseillée : { $value }.
diag-length-too-short = `{ $span }` fait { $len } octets, au moins { $min } requis.
diag-length-too-long = `{ $span }` fait { $len } octets, au plus { $max } autorisés.
diag-length-mismatch = Le champ de longueur `{ $field }` = { $value } diffère de la longueur calculée { $expected }.
diag-count-mismatch = Le champ de comptage `{ $field }` = { $value } diffère du nombre réel d'éléments { $expected }.
diag-fill-inexact = `{ $field }` ne peut pas être atteint exactement avec des éléments de remplissage : il manque { $len } octet(s).
diag-checksum-mismatch = Le champ de somme de contrôle `{ $field }` = { $value } diffère de la somme calculée { $expected }.
diag-next-protocol-mismatch = Le champ `{ $field }` = { $value } n'annonce pas la couche suivante `{ $next }` (attendu : { $expected }).
diag-auto-overflow = La valeur automatique { $value } de `{ $field }` ne tient pas sur { $bits } bits.
diag-length-not-aligned = La longueur { $len } de `{ $field }` n'est pas un multiple de { $unit } octets.
diag-no-pseudo-header = La somme de contrôle `{ $field }` nécessite un pseudo-en-tête, mais aucune couche IP n'est présente en dessous.
diag-not-implemented = `{ $field }` : { $what } n'est pas encore implémenté.
diag-truncated = La couche a été volontairement tronquée à { $len } octets.
diag-short-header = Les données se terminent dans l'en-tête { $protocol } à l'offset { $offset }.
diag-length-exceeds-data = Le champ de longueur `{ $field }` = { $len } dépasse les { $available } octets disponibles.
diag-trailing-bytes = { $len } octets restent après la fin du paquet.
diag-unsupported-field = L'analyseur ne sait pas encore décoder le champ `{ $field }` de { $protocol }.

## Descriptions des champs de protocole (wpg-protocols `desc = "..."`)
arp-hlen = Longueur de l'adresse matérielle en octets (6 pour Ethernet).
arp-htype = Type de matériel (1 = Ethernet).
arp-oper = Opération (1 = requête, 2 = réponse).
arp-plen = Longueur de l'adresse de protocole en octets (4 pour IPv4).
arp-ptype = Type de protocole (EtherType, 0x0800 = IPv4).
arp-sha = Adresse matérielle de l'émetteur.
arp-spa = Adresse de protocole de l'émetteur.
arp-tha = Adresse matérielle de la cible.
arp-tpa = Adresse de protocole de la cible.
icmp-code = Code du message, dont la signification dépend du type.
icmp-identifier = Identifiant servant à associer les requêtes et réponses echo.
icmp-rest = Champ « reste de l'en-tête » de 4 octets, propre au type.
icmp-sequence = Numéro de séquence servant à associer les requêtes et réponses echo.
icmp-type = Type de message.
ipv6-fragment-offset = Décalage de ce fragment, en unités de 8 octets.
ipv6-more-fragments = Indicateur More Fragments (1 = d'autres fragments suivent).
ipv6-options = Options TLV brutes, complétées à un multiple de 8 octets.
ipv6-routing-data = Données de routage propres au type.
ipv6-routing-type = Type d'en-tête de routage (p. ex. 2 = Mobile IPv6, 4 = SRH).
ipv6-segments-left = Nombre de segments de route restants.
srh-segments-left = Segments Left : index (dans la liste de segments) du prochain segment à visiter ; 0 au dernier.
srh-last-entry = Last Entry : index du dernier élément de la liste de segments, c.-à-d. nombre de segments - 1 (Auto).
srh-flags = Indicateurs (aucun n'est défini par la RFC 8754 ; 0).
srh-tag = Tag : marque les paquets comme appartenant à une classe ou un groupe (0 = inutilisé).
srh-segment-list = Liste de segments (SID) dans l'ordre inverse du chemin : l'élément 0 est le dernier segment, le dernier élément le premier. Tant que Segments Left est non nul, les sommes de contrôle des couches supérieures utilisent l'élément 0 comme destination.
srh-tlvs = TLV optionnels (type sur 8 bits, longueur sur 8 bits) : Pad1, PadN, HMAC (la valeur HMAC est Manuelle). L'en-tête est ensuite complété par des octets Pad1 jusqu'à un multiple de 8 (Auto).
ndp-destination = Adresse de destination d'une redirection.
ndp-managed = Indicateur de configuration d'adresse gérée (M).
ndp-options = Options NDP brutes (TLV).
ndp-other = Indicateur d'autre configuration (O).
ndp-override-flag = Indicateur Override (O).
ndp-router-flag = Indicateur Router (R).
ndp-solicited-flag = Indicateur Solicited (S).

# --- Descriptions des champs des protocoles P0 (Ethernet, VLAN, IPv4 / IPv6, TCP / UDP, charge utile) ---
eth-dst = Adresse MAC de destination.
eth-src = Adresse MAC source.
eth-ethertype = EtherType de la couche suivante (Auto depuis la couche au-dessus ; 0x88b5 = expérimental local si aucune n'est connue). Une valeur jusqu'à 0x05ff est une longueur IEEE 802.3.
dot3-length = Longueur des données LLC en octets (Auto ; au plus 1500).
llc-dsap = Point d'accès au service de destination (Auto : 0xaa = SNAP, 0xfe = OSI, 0x42 = spanning tree).
llc-ssap = Point d'accès au service source (Auto, normalement égal au DSAP).
llc-control = Champ de contrôle ; 0x03 = information non numérotée (UI).
snap-oui = Identifiant unique d'organisation (OUI) ; 0 signifie que l'identifiant de protocole est un EtherType.
snap-pid = Identifiant de protocole (un EtherType quand l'OUI vaut 0 ; Auto depuis la couche suivante).
vlan-pcp = Priority code point (classe de service 802.1p, 0 à 7).
vlan-dei = Indicateur d'éligibilité à l'abandon (DEI).
vlan-vid = Identifiant de VLAN (1 à 4094 ; 0 = étiquette de priorité seule, 4095 est réservé).
vlan-ethertype = EtherType de la couche après l'étiquette (Auto ; 0x8100 pour une autre étiquette 802.1Q).
dot1ad-ethertype = EtherType de la couche après l'étiquette S (Auto ; normalement 0x8100 pour l'étiquette C 802.1Q).
ipv4-version = Version IP, toujours 4.
ipv4-ihl = Longueur de l'en-tête en mots de 32 bits, 5 à 15 (Auto, options comprises).
ip-dscp = Differentiated services code point (classe QoS, 0 à 63).
ip-ecn = Notification explicite de congestion (0 = non ECT, 1 / 2 = ECT, 3 = CE).
ipv4-total-length = Longueur totale de l'en-tête et de la charge utile en octets (Auto).
ipv4-identification = Identification partagée par les fragments d'un même datagramme.
ipv4-flags = Drapeaux : DF (ne pas fragmenter) et MF (fragments suivants) ; le bit de poids fort est réservé.
ipv4-frag-offset = Décalage de ce fragment en unités de 8 octets.
ipv4-ttl = Durée de vie ; décrémentée à chaque routeur, 0 n'est jamais envoyé.
ipv4-protocol = Numéro de protocole de la couche suivante (Auto ; 253 = expérimental si aucune n'est connue).
ipv4-header-checksum = Somme de contrôle Internet de l'en-tête (Auto).
ipv4-src = Adresse IPv4 source.
ipv4-dst = Adresse IPv4 de destination.
ipv4-options = Options IPv4 en TLV (EOL, NOP, sécurité, enregistrement de route, horodatage...).
ipv4-padding = Remplissage à zéro des options jusqu'à une frontière de 32 bits (Auto).
udp-src-port = Port source.
udp-dst-port = Port de destination ; sélectionne la couche suivante quand il est Auto (53 = DNS, 67 / 68 = DHCP, 4789 = VXLAN...).
udp-length = Longueur de l'en-tête et de la charge utile en octets (Auto).
udp-checksum = Somme de contrôle Internet avec le pseudo-en-tête IPv4 / IPv6 (Auto ; 0 devient 0xffff).
tcp-src-port = Port source.
tcp-dst-port = Port de destination ; sélectionne la couche suivante (179 = BGP, 646 = LDP, 3868 = Diameter, 5060 = SIP).
tcp-seq = Numéro de séquence du premier octet de données (numéro de séquence initial avec SYN).
tcp-ack = Numéro d'acquittement, valide quand le drapeau ACK est levé.
tcp-data-offset = Longueur de l'en-tête en mots de 32 bits, 5 à 15 (Auto, options comprises).
tcp-reserved = Bits réservés, 0.
tcp-flags = Drapeaux de contrôle : CWR, ECE, URG, ACK, PSH, RST, SYN, FIN (SYN par défaut).
tcp-window = Fenêtre de réception en octets (multipliée par l'option window scale).
tcp-checksum = Somme de contrôle Internet avec le pseudo-en-tête IPv4 / IPv6 (Auto).
tcp-urgent-ptr = Pointeur urgent, valide quand le drapeau URG est levé.
tcp-options = Options TCP en TLV : EOL, NOP, MSS, window scale, SACK permitted, SACK, timestamps.
tcp-padding = Remplissage à zéro des options jusqu'à une frontière de 32 bits (Auto).
payload-mode = Génération de la charge utile : fixed (données telles quelles), pattern (données répétées jusqu'à la longueur), random (longueur octets depuis la graine) ou counter (données avec un compteur par paquet intégré).
payload-data = Octets de la charge utile ; en mode fixed tronqués ou complétés par des zéros jusqu'à la longueur si elle est non nulle, en mode pattern l'unité répétée (vide = 00 01 02 ...).
payload-length = Longueur de la charge utile en octets ; 0 conserve la longueur des données (mode fixed / counter).
payload-seed = Graine du générateur pseudo-aléatoire en mode random.
payload-counter-offset = Décalage en octets du compteur intégré en mode counter.
payload-counter-width = Largeur du compteur intégré : 4 ou 8 octets, gros-boutiste.
payload-counter-value = Valeur du compteur écrite par paquet par l'expansion de flux (mode counter).
raw-data = Octets bruts jusqu'à la fin de la trame.
ipv6-version = Version IP, toujours 6.
ipv6-flow-label = Étiquette de flux (20 bits) identifiant un flux pour les routeurs ; 0 = aucune.
ipv6-payload-length = Longueur de tout ce qui suit cet en-tête, en-têtes d'extension compris (Auto).
ipv6-next-header = Numéro de protocole de l'en-tête suivant (Auto ; 253 = expérimental si aucun n'est connu).
ipv6-hop-limit = Limite de sauts ; décrémentée à chaque routeur, 0 n'est jamais envoyé.
ipv6-src = Adresse IPv6 source.
ipv6-dst = Adresse IPv6 de destination.
ipv6-ext-next-header = Numéro de protocole de l'en-tête qui suit cet en-tête d'extension (Auto).
ipv6-hdr-ext-len = Longueur de cet en-tête d'extension en unités de 8 octets, sans les 8 premiers octets (Auto).
ipv6-ext-padding = Remplissage à zéro (Pad1) de l'en-tête d'extension jusqu'à un multiple de 8 octets (Auto).
field-reserved = Réservé, 0.
ipv6-frag-identification = Identification partagée par les fragments d'un même paquet.
icmp-checksum = Somme de contrôle Internet de l'en-tête et de la charge utile (Auto).
icmpv6-checksum = Somme de contrôle Internet avec le pseudo-en-tête IPv6 sur l'en-tête et la charge utile (Auto).
ndp-cur-hop-limit = Limite de sauts que les hôtes doivent utiliser (0 = non spécifiée).
ndp-router-lifetime = Durée de vie comme routeur par défaut en secondes (0 = pas un routeur par défaut).
ndp-reachable-time = Temps d'accessibilité en millisecondes (0 = non spécifié).
ndp-retrans-timer = Temporisateur de retransmission en millisecondes (0 = non spécifié).
ndp-target = Adresse cible.

## wpg ifaces
ifaces-col-name = NOM
ifaces-col-index = IDX
ifaces-col-mac = MAC
ifaces-col-ipv4 = IPV4
ifaces-col-ipv6 = IPV6
ifaces-col-mtu = MTU
ifaces-col-state = ÉTAT
ifaces-col-speed = DÉBIT
ifaces-col-driver = PILOTE
ifaces-state-up = active
ifaces-state-down = inactive
ifaces-state-no-carrier = pas de porteuse
ifaces-state-loopback = bouclage
ifaces-none = Aucune interface réseau trouvée.
ifaces-error = Impossible de lister les interfaces réseau : { $error }

## wpg doctor
doctor-title = wpg doctor : vérification de l'environnement
doctor-summary = { $ok } ok, { $warn } avertissement(s), { $fail } échec(s)
doctor-skip = Non applicable sur ce système d'exploitation
doctor-caps-ok = Les sockets bruts sont disponibles
doctor-caps-missing = Privilèges insuffisants pour l'émission de paquets bruts (CAP_NET_RAW / root / Administrateur)
doctor-caps-npcap-user = Pas en mode élevé, mais Npcap autorise la capture et l'envoi aux non-administrateurs
doctor-caps-npcap-admin = Npcap est réservé aux administrateurs ; lancez wpg depuis un terminal élevé pour envoyer
doctor-fix-caps = Accordez le privilège : { $hint }
doctor-iface-ok = Au moins une interface réseau utilisable est active
doctor-iface-none = Aucune interface autre que le bouclage n'est active avec porteuse
doctor-iface-error = Impossible d'énumérer les interfaces réseau
doctor-fix-iface = Inspectez les interfaces : { $hint }
doctor-af-xdp-ok = Le noyau prend en charge AF_XDP (5.10 ou plus récent)
doctor-af-xdp-old = Le noyau est trop ancien pour AF_XDP (5.10 ou plus récent requis) ; le backend afpacket fonctionne toujours
doctor-tpacket-v3-ok = Le noyau prend en charge TPACKET_V3
doctor-tpacket-v3-old = Le noyau ne prend pas en charge TPACKET_V3 ; afpacket se rabat sur sendmmsg
doctor-hugepages-ok = Des hugepages sont configurées
doctor-hugepages-none = Aucune hugepage configurée (nécessaire uniquement pour le backend DPDK)
doctor-fix-hugepages = Réservez des hugepages et liez la carte réseau à DPDK : { $hint }
doctor-npcap-ok = Npcap est installé
doctor-npcap-missing = Npcap n'est pas installé ; il est nécessaire pour émettre sous Windows
doctor-npcap-no-dll = Npcap semble installé, mais wpcap.dll est introuvable dans System32\Npcap comme dans System32
doctor-fix-npcap = Installez (ou réinstallez) Npcap, puis relancez wpg doctor : { $hint }
doctor-pcaplib-ok = libpcap est disponible
doctor-pcaplib-missing = libpcap est introuvable ; le backend pcap ne peut pas être utilisé
doctor-fix-pcaplib = Installez libpcap : { $hint }

## wpg template lint (clés `LintIssue` de wpg-template)
lint-level-error = erreur
lint-level-warning = avertissement
lint-schema-version = Version de schéma { $value } non prise en charge (prises en charge : { $expected }).
lint-missing-name = `meta.name` est vide.
lint-no-layers = Le modèle n'a aucune couche.
lint-unknown-protocol = Protocole inconnu `{ $protocol }`.
lint-unknown-field = Le protocole `{ $protocol }` n'a pas de champ `{ $field }`.
lint-invalid-value = Valeur invalide pour `{ $field }` : { $value }
lint-invalid-generator = Générateur invalide pour `{ $field }` : { $value }
lint-unknown-variable = Variable inconnue `{ $value }`.
lint-missing-variable = La variable `{ $value }` n'a pas de valeur par défaut et aucune valeur n'a été fournie.
lint-unused-variable = La variable `{ $value }` n'est jamais utilisée.
lint-duplicate-variable = La variable `{ $value }` est déclarée plusieurs fois.
lint-var-syntax = Espace réservé de variable non terminé dans `{ $value }`.
lint-override-unknown = Surcharge de variable inconnue `{ $value }`.
lint-field-and-generator = `{ $field }` a à la fois une valeur fixe et un générateur ; le générateur l'emporte.
lint-next-protocol-manual = `{ $next }` ne figure pas dans la table de liaison de `{ $protocol }` ; `{ $field }` sera Manuel.
lint-unknown-order = `{ $value }` dans l'ordre du flux n'a pas de générateur.
lint-invalid-stream = Paramètre de flux invalide : { $value }
lint-build-error = Impossible de construire le paquet : { $value }

## wpg template / wpg render
template-lint-ok = { $value } : aucun problème
template-lint-load-error = { $value } : { $error }
template-lint-summary = { $fail } erreur(s), { $warn } avertissement(s) dans { $ok } fichier(s)
template-list-empty = Aucun modèle trouvé dans { $value }.
template-show-hex = Vidage hexadécimal ({ $len } octets)
template-show-layout = Structure
render-done = { $len } paquets écrits dans { $value }.
render-unbounded = Le flux est illimité ; indiquez --count N.
cli-bad-var = --var `{ $value }` invalide ; format attendu : NOM=VALEUR.

## wpg send / wpg bench (progression, résumé, erreurs)
tx-progress = [{ $t } s] { $packets } paq.  { $pps }pps  { $bps }bps  perdus { $dropped }
tx-progress-rate = débit { $ratio }%
tx-progress-paused = (en pause)
tx-progress-scheduled = (planifié, démarre dans { $secs } s)
tx-interrupted = Interrompu, arrêt en cours...
tx-ctrlc-warning = Avertissement : impossible d'installer le gestionnaire Ctrl-C : { $error }
tx-open-failed = Impossible d'ouvrir { $backend } sur `{ $iface }` : { $error }
tx-start-failed = Impossible de démarrer la tâche : { $error }
tx-available-backends = Backends disponibles : { $value }
tx-iface-required = --iface est requis pour le backend { $backend }.
tx-iface-required-pcapfile = --iface <chemin de sortie> est requis pour le backend pcapfile.
tx-bad-workers = --workers doit valoir au moins 1.
tx-workers-unsupported = Le backend { $backend } ne prend en charge qu'un seul worker ; utilisez --workers 1.
tx-pin-cores-short = Avertissement : --pin-cores liste { $count } cœur(s) pour { $workers } workers ; les autres ne sont pas épinglés.
tx-json-error = Impossible d'écrire le JSON : { $error }
send-bad-rate = --rate invalide : { $error }
send-bad-duration = --duration invalide : { $error }
send-bad-ramp-up = --ramp-up invalide : { $error }
send-bad-start-at = --start-at invalide : { $error }
send-link-speed-unknown = Impossible de déterminer le débit du lien de `{ $iface }` pour `{ $value }%` ; indiquez un débit absolu tel que `1Gbps`.
send-bad-plan = --plan doit valoir auto, prerender ou onthefly.
send-stream-error = Erreur de flux : { $error }
send-render-error = Erreur de rendu : { $error }
send-starting = Émission sur { $iface } via { $backend }{ $detail } : débit { $rate }, rafale { $burst }, { $limit }, { $workers } worker(s), plan { $strategy } ({ $reason })
send-limit-count = { $count } paquets
send-limit-count-or-duration = { $count } paquets ou { $duration }
send-limit-duration = pendant { $duration }
send-limit-forever = jusqu'à Ctrl-C
send-ramp-up = Montée en charge : de 0 à { $rate } en { $duration }
send-scheduled = Démarrage planifié à { $time } (dans { $secs } s) ; Ctrl-C pour annuler.
summary-state = état       { $value }
summary-state-finished = terminé ({ $reason })
summary-state-failed = échec : { $error }
summary-reason-count-reached = nombre atteint
summary-reason-duration-elapsed = durée écoulée
summary-reason-source-exhausted = flux épuisé
summary-reason-stopped = arrêté
summary-backend = backend    { $backend }{ $detail }
summary-workers = workers    { $workers }
summary-packets = paquets    { $value }
summary-bytes = octets     { $value }
summary-dropped = perdus     { $value }
summary-elapsed = écoulé     { $secs } s
summary-average = moyenne    { $pps }pps  { $bps }bps (L2)  { $l1 }bps (L1)
summary-accuracy = précision  cible { $target }{ $unit }  atteint { $ratio }%  gigue max { $max }  gigue moyenne { $mean }
summary-stream = flux { $index }   { $packets } paq.  { $bytes } octets
summary-worker = worker { $index }   { $packets } paq.  { $bytes } octets{ $core }
summary-worker-core = , cœur { $core }
summary-warning = alerte     { $error }
summary-unsigned = non signés { $value } (pas de place pour une signature)

## wpg capture (réception / vérification)
rx-progress = [{ $t } s] { $packets } paq.  { $pps }pps  { $bps }bps  correspondants { $matched } ({ $mpps }pps)  perdus { $dropped }
capture-starting = Capture sur { $iface } via { $backend }, filtre { $filter } (Ctrl-C pour arrêter)
capture-filter-all = (toutes les trames)
capture-iface-required-pcapfile = --iface <fichier de capture> est requis pour le backend pcapfile.
capture-bad-filter = --filter invalide : { $error }
capture-running = en cours
capture-reason-end-of-input = fin des données d'entrée
capture-packet = #{ $index }  { $time }  { $len } o  { $layers }{ $signature }
capture-packet-signature = { "  " }[wpg flux { $stream } seq { $seq }]
capture-summary-matched = retenus   { $packets } paq.  { $bytes } octets
capture-summary-average = moyenne    { $pps }pps  { $bps }bps  retenus { $mpps }pps  { $mbps }bps
capture-summary-saved = enregistrés { $count } paq. dans { $path }
capture-summary-signed = signés     { $signed } avec signature, { $unsigned } sans
capture-summary-stream = flux { $stream } reçus { $received }  seq { $first }-{ $last }  perdus { $lost } ({ $ratio }%)  désordonnés { $reordered }  doublons { $duplicates }
capture-summary-latency = { "             " }latence min { $min }  moy { $avg }  max { $max }  gigue { $jitter }

bench-iface = interface  { $iface }
bench-worker = worker { $index }   { $detail }
bench-frame = trame      { $len } o (+4 FCS)  lot { $batch }
bench-template = modèle     { $value } (à la volée, { $mode })
bench-mode-patch = modifié sur place
bench-mode-rebuild = reconstruction complète par trame
bench-rate = débit      { $pps }pps
bench-bits = { $bps }bps (L2)  { $l1 }bps (L1)
bench-bad-duration = --duration doit être un nombre de secondes positif ou nul.
bench-failed = Échec du benchmark : { $error }
import-done = { $len } paquet(s) importé(s) dans { $count } modèle(s) : { $value }
import-read-error = { $value } : { $error }
import-write-error = { $value } : { $error }
import-note = { $value }
import-note-packet = paquet { $index } : { $value }
import-packet-problems = le paquet { $index } présente { $count } problème(s) d'analyse
import-packet-skipped = le paquet { $index } n'a pas pu être décodé et a été ignoré
import-not-reproducible = Le modèle importé ne reconstruit pas les octets capturés ; la trame a été enregistrée en données brutes.
import-mixed-stacks = { $count } paquet(s) ont une pile de couches différente du premier ; aucun générateur n'a été déduit.
import-generator-approximate = { $value } n'est qu'une approximation des valeurs capturées.
import-generators-dropped = Les générateurs déduits produisaient un flux invalide et ont été abandonnés.
template-export-none = Aucun modèle à exporter n'a été indiqué.
template-export-not-found = Modèle introuvable : { $value }
template-export-done = { $count } modèle(s) exporté(s) vers { $value }.
template-export-error = { $value } : { $error }
template-import-done = { $count } modèle(s) extrait(s) de { $value } vers { $detail }.
template-import-exists = { $value } : existe déjà (utilisez --force pour écraser)
template-import-error = { $value } : { $error }
template-migrate-outdated = { $value } : schéma { $found }, migration vers { $expected } nécessaire
template-migrate-done = { $value } : migré du schéma { $found } vers { $expected }
template-migrate-error = { $value } : { $error }
template-migrate-summary = { $count } migré(s), { $ok } déjà au schéma { $expected }, { $fail } erreur(s)
template-migrate-check-summary = { $count } à migrer, { $ok } déjà au schéma { $expected }, { $fail } erreur(s)

## Erreurs de l'API (wpg-server)
api-error-not-found = { $what } est introuvable.
api-error-invalid-body = Corps de requête invalide : { $error }
api-error-invalid-query = Paramètre de requête invalide : { $error }
api-error-invalid-template = Modèle invalide : { $error }
api-error-invalid-hex = Entrée hexadécimale invalide : { $error }
api-error-parse-failed = Les octets n'ont pas pu être décodés : { $error }
api-error-render-failed = Échec du rendu : { $error }
api-error-count-range = `count` doit être compris entre 1 et { $max } (reçu : { $error }).
api-error-read-only = Le modèle { $id } appartient à une bibliothèque en lecture seule ; dupliquez-le pour le modifier.
api-error-policy-iface = L'émission sur { $iface } n'est pas autorisée par la politique du serveur (autorisées : { $allowed }).
api-error-policy-rate = Le débit { $rate } dépasse le maximum autorisé par la politique du serveur ({ $max }).
api-error-policy-unlimited = Un débit illimité n'est pas autorisé par la politique du serveur (maximum { $max }).
api-error-invalid-state = La tâche { $id } est { $state } ; impossible de { $action }.
api-error-job-spec = Tâche invalide : { $error }
api-error-backend = Erreur du backend d'émission : { $error }
api-error-capture-spec = Capture invalide : { $error }
api-error-capture-filter = Filtre de capture invalide : { $error }
api-error-capture-backend = Erreur du backend de réception : { $error }
api-error-capture-state = La capture { $id } est { $state } ; impossible de { $action }.
api-error-internal = Erreur interne du serveur : { $error }
auth-error-unauthorized = Veuillez vous connecter.
auth-error-bad-credentials = Nom d'utilisateur ou mot de passe incorrect.
auth-error-setup-required = Aucun mot de passe administrateur n'est encore défini : terminez d'abord la configuration initiale.
auth-error-already-configured = Le mot de passe administrateur a déjà été défini.
auth-error-rate-limited = Trop d'échecs de connexion. Réessayez dans { $secs } secondes.
auth-error-csrf = Jeton CSRF manquant ou invalide (en-tête X-CSRF-Token).
auth-error-origin = Les connexions WebSocket depuis une autre origine ne sont pas autorisées.
auth-error-setup-remote = Le mot de passe administrateur ne peut être défini que depuis le serveur lui-même (localhost).
auth-error-password-mismatch = Les mots de passe ne correspondent pas.
auth-error-password-short = Le mot de passe doit comporter au moins { $min } caractères.
lint-parse-error = Le modèle ne peut pas être analysé : { $error }

## Configuration (wpg.toml)
config-issue-file-missing = fichier introuvable
config-issue-bad-url = doit commencer par http:// ou https://
config-issue-not-argon2 = ne ressemble pas à un hachage argon2 ($argon2id$...)
config-issue-password-both = password_hash et password_file sont tous deux définis ; password_hash l'emporte
config-issue-zero = doit être supérieur à 0
config-issue-exposed-without-password = écoute sur une adresse autre que le bouclage sans mot de passe
config-issue-dir-missing = répertoire introuvable
config-issue-protocols = impossible de charger les protocoles définis par l'utilisateur
protocols-ext-loaded = { $count } protocole(s) défini(s) par l'utilisateur chargé(s)
protocols-ext-error = impossible de charger les protocoles définis par l'utilisateur : { $file } : { $error }
config-issue-max-rate-percent = un pourcentage du lien ne peut pas servir de maximum global du serveur ; utilisez pps ou bps
config-issue-bad-rate = débit invalide
config-issue-unknown-backend = backend inconnu
config-issue-backend-unavailable = backend indisponible dans cette compilation / sur ce système
config-issue-bad-log-level = filtre de journal invalide
config-level-error = erreur
config-level-warning = avertissement
config-issue-line = { $level } : { $key } : { $message }{ $detail }
config-load-error = Erreur de configuration : { $error }
config-init-written = { $path } a été écrit. Modifiez-le, puis vérifiez-le avec `wpg config validate`.
config-init-exists = { $path } existe déjà (utilisez --force pour écraser).
config-validate-ok = Configuration correcte ({ $source }).
config-validate-summary = { $errors } erreur(s), { $warnings } avertissement(s) ({ $source }).
config-source-defaults = valeurs par défaut intégrées et environnement
config-password-prompt = Nouveau mot de passe administrateur :{ " " }
config-password-confirm = Répétez le mot de passe :{ " " }
config-password-mismatch = Les mots de passe ne correspondent pas.
config-password-short = Le mot de passe doit comporter au moins { $min } caractères.
config-password-written = Le hachage du mot de passe a été enregistré dans { $path } (auth.password_hash). Redémarrez `wpg serve` pour l'appliquer.

## wpg serve
serve-banner-title = { $app } { $version }
serve-banner-listen = Écoute          { $url }/
serve-banner-docs = Doc. de l'API   { $url }/api/docs/
serve-banner-config = Configuration   { $source }
serve-banner-data = Données         { $path }
serve-banner-library = Bibliothèque    { $path }{ $state }
serve-banner-user-dir = Modèles utilis. { $path }
serve-banner-templates = Modèles         { $count }
serve-banner-backends = Backends        { $list }
serve-banner-missing = { " " }(absent)
serve-banner-dev = Mode développement : CORS autorisé pour { $origins }
serve-banner-no-password = Aucun mot de passe n'est configuré et le serveur écoute sur { $url } (--allow-insecure) : toute personne pouvant l'atteindre peut définir le mot de passe administrateur et envoyer des paquets. Définissez un mot de passe (wpg config set-password) dès que possible.
serve-failed = Erreur du serveur : { $error }
serve-log-error = Impossible de configurer la journalisation : { $error }
serve-banner-tls = TLS             { $path }
serve-banner-tls-self-signed = TLS             { $path } (auto-signé)
serve-banner-fingerprint = SHA-256         { $fingerprint }
serve-banner-auth = Connexion       { $state }
serve-banner-auth-setup = pas encore de mot de passe : définissez-le sur l'écran de configuration
serve-banner-auth-configured = mot de passe administrateur issu de { $source }
serve-banner-auth-database = mot de passe administrateur défini sur l'écran de configuration
serve-banner-setup-local = Aucun mot de passe administrateur n'est encore défini. Ouvrez { $url } dans un navigateur sur cette machine pour le définir.
serve-refuse-insecure = Refus d'écouter sur { $addr } : aucun mot de passe administrateur n'est configuré. Utilisez une adresse de bouclage (127.0.0.1), définissez un mot de passe (--password-file, WPG_PASSWORD, wpg config set-password) ou passez --allow-insecure.
serve-password-arg-warning = Avertissement : --password est visible des autres utilisateurs dans la liste des processus ; préférez --password-file, WPG_PASSWORD ou `wpg config set-password`.

## Interface web (wpg-web). Les clés sont vérifiées par le test `ui_keys_exist` dans crates/wpg-web.
ui-lang-ja = 日本語
ui-lang-en = English
ui-lang-zh-cn = 简体中文
ui-lang-zh-tw = 繁體中文
ui-lang-ko = 한국어
ui-lang-de = Deutsch
ui-lang-fr = Français
ui-lang-es = Español
ui-loading = Chargement…
ui-close = Fermer
ui-cancel = Annuler
ui-delete = Supprimer
ui-save = Enregistrer
ui-refresh = Actualiser
ui-retry = Réessayer
ui-yes = Oui
ui-no = Non
ui-unlimited = Illimité
ui-help = Raccourcis clavier
ui-help-title = Raccourcis clavier
ui-skip-to-content = Aller au contenu
ui-not-found = Cette page n'existe pas.
ui-back-to-dashboard = Retour au tableau de bord
ui-view-only = Écran étroit : consultation uniquement. L'édition est disponible sur les écrans plus larges.
ui-server-unreachable = Le serveur est injoignable. `wpg serve` est-il lancé ?
ui-error-http = Le serveur a répondu HTTP { $id }.
ui-error-network = Erreur réseau : { $error }
ui-error-decode = Réponse inattendue du serveur : { $error }
ui-nav-label = Navigation principale
ui-nav-dashboard = Tableau de bord
ui-nav-builder = Éditeur
ui-nav-jobs = Tâches
ui-nav-templates = Modèles
ui-nav-rx = Réception
ui-nav-settings = Paramètres
ui-shortcut-hint = affiche les raccourcis clavier
ui-shortcut-dashboard = Aller au tableau de bord
ui-shortcut-builder = Aller à l'éditeur
ui-shortcut-jobs = Aller aux tâches
ui-shortcut-templates = Aller aux modèles
ui-shortcut-rx = Aller à la réception
ui-shortcut-settings = Aller aux paramètres
ui-shortcut-new-job = Nouvelle tâche
ui-shortcut-logs = Afficher / masquer le journal du serveur
ui-shortcut-help = Afficher cette aide
ui-shortcut-escape = Fermer la boîte de dialogue
ui-ws-connecting = Connexion…
ui-ws-open = En direct
ui-ws-closed = Déconnecté (nouvelle tentative)
ui-theme = Thème
ui-theme-light = Clair
ui-theme-dark = Sombre
ui-theme-system = Système
ui-theme-to-dark = Passer au thème sombre
ui-theme-to-light = Passer au thème clair
ui-logout = Se déconnecter
ui-logs-toggle = Journal du serveur
ui-logs-title = Journal du serveur
ui-logs-level = Niveau
ui-logs-follow = Suivre
ui-logs-clear = Effacer le journal
ui-logs-count = { $count } lignes
ui-logs-empty = Aucune ligne de journal pour l'instant.
ui-login-title = Connexion
ui-login-submit = Se connecter
ui-login-busy = Connexion…
ui-login-wrong = Mot de passe incorrect.
ui-login-rate-limited = Trop de tentatives. Réessayez dans { $secs } s.
ui-login-wait = Patientez { $secs } s
ui-password = Mot de passe
ui-password-confirm = Confirmer le mot de passe
ui-setup-title = Définir le mot de passe administrateur
ui-setup-intro = Ce serveur n'a pas encore de mot de passe. Choisissez-en un pour protéger l'interface web et l'API.
ui-setup-hint = Au moins { $min } caractères.
ui-setup-too-short = Le mot de passe doit comporter au moins { $min } caractères.
ui-setup-mismatch = Les mots de passe ne correspondent pas.
ui-setup-submit = Définir le mot de passe
ui-dash-totals = Totaux
ui-dash-active-jobs = Tâches actives
ui-dash-running-jobs = En émission
ui-dash-total-pps = pps total
ui-dash-total-bps = Débit total
ui-dash-templates = Modèles
ui-dash-uptime = Disponibilité du serveur
ui-dash-running = Tâches en cours
ui-dash-no-running = Aucune tâche en cours.
ui-dash-doctor = Autodiagnostic
ui-dash-ifaces = Interfaces
ui-doctor-ok = Toutes les vérifications ont réussi
ui-doctor-warn = Avertissements
ui-doctor-fail = Problèmes détectés
ui-doctor-counts = { $ok } OK · { $warn } avertissements · { $fail } échecs
ui-doctor-details = Rapport complet
ui-doctor-status = Statut
ui-doctor-check = Vérification
ui-doctor-detail = Détail / correction
ui-doctor-status-ok = OK
ui-doctor-status-warn = Avertissement
ui-doctor-status-fail = Échec
ui-doctor-status-skip = Ignoré
ui-iface-name = Nom
ui-iface-state = État
ui-iface-mac = MAC
ui-iface-addrs = Adresses
ui-iface-mtu = MTU
ui-iface-speed = Débit
ui-iface-allowed = Autorisée en émission
ui-iface-up = Active
ui-iface-down = Inactive
ui-iface-no-carrier = Pas de porteuse
ui-stat-pps = Paquets/s
ui-stat-bps = Débit (L2)
ui-stat-l1-bps = Débit (L1)
ui-stat-packets = Paquets
ui-stat-bytes = Octets
ui-stat-dropped = Perdus
ui-stat-ratio = Débit atteint
ui-stat-elapsed = Écoulé
ui-spark-pps = Paquets par seconde, 30 dernières secondes
ui-state-created = Créée
ui-state-scheduled = Planifiée
ui-state-running = En cours
ui-state-paused = En pause
ui-state-finished = Terminée
ui-state-failed = Échec
ui-state-unknown = Inconnu
ui-reason-count-reached = Nombre de paquets atteint
ui-reason-duration-elapsed = Durée écoulée
ui-reason-source-exhausted = Tous les paquets ont été envoyés
ui-reason-stopped = Arrêtée
ui-jobs-new = Nouvelle tâche
ui-jobs-tab-live = En cours
ui-jobs-tab-history = Historique
ui-jobs-empty = Aucune tâche.
ui-jobs-streams = Flux
ui-jobs-template = Modèle
ui-jobs-choose-template = Choisissez un modèle…
ui-jobs-weight = Poids
ui-jobs-add-stream = Ajouter un flux
ui-jobs-remove-stream = Supprimer le flux
ui-jobs-mix = Mélange
ui-mix-weighted = Pondéré
ui-mix-sequential = Séquentiel
ui-mix-round-robin = Tourniquet
ui-jobs-target = Où
ui-jobs-backend = Backend
ui-jobs-iface = Interface
ui-jobs-choose-iface = Choisissez une interface…
ui-jobs-iface-not-needed = Inutile pour ce backend
ui-jobs-iface-denied = non autorisée
ui-jobs-dry-run = Simulation : nom du fichier pcapng
ui-jobs-dry-run-hint = Si renseigné, rien n'est émis : les paquets sont écrits dans ce fichier, dans le répertoire de capture du serveur.
ui-jobs-how = Comment
ui-jobs-rate = Débit
ui-jobs-count = Nombre de paquets
ui-jobs-duration = Durée
ui-jobs-burst = Rafale
ui-jobs-workers = Workers
ui-jobs-workers-disabled = Plusieurs workers nécessitent un backend multi-worker et un seul flux.
ui-jobs-from-template = Selon le modèle
ui-jobs-name = Nom
ui-jobs-name-placeholder = Par défaut, le nom du modèle
ui-jobs-start-now = Démarrer immédiatement
ui-jobs-create = Créer la tâche
ui-jobs-create-start = Créer et démarrer
ui-jobs-created = Tâche « { $id } » créée.
ui-jobs-need-iface = Choisissez une interface.
ui-jobs-need-template = Choisissez un modèle pour chaque flux.
ui-jobs-bad-weight = Les poids doivent être des entiers positifs.
ui-rate-default = Vide : le débit du modèle (ou la limite du serveur).
ui-rate-is-pps = → { $rate }
ui-rate-is-bps = → { $rate } sur le support (couche 1)
ui-rate-is-percent = → { $rate } % du débit du lien
ui-rate-is-ifg = Intervalle fixe entre les trames : { $rate }
ui-rate-is-unlimited = Aussi vite que possible
ui-valid-empty = Obligatoire.
ui-valid-number = Ce n'est pas un nombre.
ui-valid-integer = Saisissez un nombre entier.
ui-valid-range = Doit être supérieur à zéro.
ui-valid-percent = Un pourcentage ne peut pas dépasser 100.
ui-valid-rate-unit = Unité inconnue : utilisez pps, kpps, Mpps, bps, kbps, Mbps, Gbps, %, ifg=<durée> ou unlimited.
ui-valid-duration-unit = Unité inconnue : utilisez ns, us, ms, s, m ou h.
ui-valid-capture-name = Utilisez un nom de fichier relatif sans « .. ».
ui-confirm-send-title = Émettre sur une interface réelle ?
ui-confirm-send-body = Cette tâche émet des paquets sur le réseau. Vérifiez que l'interface et le débit conviennent à ce réseau.
ui-confirm-send = Émettre
ui-job-detail = Tâche
ui-job-state = État
ui-job-start = Démarrer
ui-job-pause = Pause
ui-job-resume = Reprendre
ui-job-stop = Arrêter
ui-job-download-report = Rapport (JSON)
ui-job-live = Statistiques en direct
ui-job-streams = Par flux
ui-job-inline-template = (modèle intégré)
ui-job-workers = Par worker
ui-job-workers-pending = Les compteurs par worker font partie du rapport final, disponible à la fin de la tâche.
ui-job-active = Temps actif
ui-job-core = Cœur
ui-job-accuracy = Précision du cadencement
ui-acc-target = Cible
ui-acc-achieved = Atteint
ui-acc-jitter = Gigue (moyenne / max)
ui-job-spec = Paramètres
ui-job-created = Créée
ui-job-started = Démarrée
ui-job-finished = Terminée
ui-job-deleted = Tâche supprimée.
ui-job-delete-title = Supprimer cette tâche ?
ui-job-delete-body = Une tâche en cours est d'abord arrêtée ; son rapport reste dans l'historique.
ui-tpl-import = Importer
ui-tpl-new = Nouveau modèle
ui-tpl-search = Rechercher un nom, une description, un tag, un protocole…
ui-tpl-source = Source
ui-tpl-source-all = Tous
ui-tpl-source-library = Bibliothèque
ui-tpl-source-user = Les miens
ui-tpl-favorites-only = Favoris uniquement
ui-tpl-tags = Tags
ui-tpl-recent = Utilisés récemment
ui-tpl-empty = Aucun modèle ne correspond.
ui-tpl-favorite = Favori
ui-tpl-view = Afficher / modifier le YAML
ui-tpl-open-builder = Ouvrir dans l'éditeur
ui-tpl-new-job = Nouvelle tâche avec ce modèle
ui-tpl-export = Télécharger le YAML
ui-tpl-duplicate = Dupliquer
ui-tpl-imported = Modèle « { $id } » importé.
ui-tpl-duplicated = Copie « { $id } » créée.
ui-tpl-deleted = Modèle « { $id } » supprimé.
ui-tpl-delete-title = Supprimer ce modèle ?
ui-tpl-delete-body = « { $id } » sera supprimé de votre répertoire de modèles.
ui-tpl-saved = Modèle enregistré.
ui-tpl-yaml = YAML du modèle
ui-tpl-read-only = Les modèles de la bibliothèque sont en lecture seule : dupliquez-le pour modifier une copie.
ui-lint-run = Vérifier (lint)
ui-lint-clean = Aucun problème détecté.
ui-lint-summary = { $errors } erreurs, { $warnings } avertissements
ui-set-appearance = Apparence
ui-set-account = Compte
ui-set-logged-in = Connecté en tant que { $id }.
ui-set-auth-disabled = L'authentification n'est pas activée sur ce serveur.
ui-set-client = Ce navigateur
ui-set-client-note = Enregistré uniquement dans ce navigateur. La configuration du serveur (wpg.toml) n'est pas exposée par l'API.
ui-set-api = API
ui-set-ws = Événements
ui-set-ui-version = Version de l'interface
ui-set-api-docs = Documentation de l'API (Swagger UI)
ui-set-about = À propos
ui-set-server-version = Version du serveur
ui-set-api-version = Version de l'API
ui-set-platform = Plateforme
ui-set-features = Fonctionnalités optionnelles
ui-set-locales = Langues
ui-set-license = wpg est distribué sous double licence MIT ou Apache-2.0.
ui-builder-placeholder = L'éditeur de paquets (palette de protocoles, pile de couches, éditeur de champs, vue hexadécimale / arborescente) arrive à la prochaine étape du jalon.
ui-builder-placeholder-template = L'éditeur de paquets arrive à la prochaine étape du jalon. Le modèle « { $id } » s'ouvrira ici.
ui-reason-end-of-input = Fin du fichier de capture
ui-valid-stream-id = Saisissez un nombre entier de 0 à 4294967295.
ui-jobs-verification = Vérification en réception
ui-jobs-signature = Apposer une signature de vérification
ui-jobs-signature-hint = Écrit un identifiant de flux, un numéro de séquence et l'heure d'émission dans les 24 derniers octets de charge utile de chaque trame, afin qu'une capture avec « Vérifier les signatures » puisse mesurer les pertes, l'ordre et la latence.
ui-jobs-stream-id = Id. de flux de la signature (premier flux)
ui-job-signature = Signature
ui-job-signature-on = Activée (id. de flux { $id } et suivants)
ui-rx-new = Nouvelle capture
ui-rx-file = Fichier de capture
ui-rx-file-hint = Nom d'un fichier pcap / pcapng dans le répertoire de capture du serveur, p. ex. la sortie d'une simulation de tâche.
ui-rx-iface-any = any (toutes les interfaces)
ui-rx-filter = Filtre
ui-rx-filter-empty = Vide : toutes les trames correspondent.
ui-rx-filter-checking = Vérification du filtre…
ui-rx-filter-ok = Valide : { $filter }
ui-rx-filter-error = { $error } (au caractère { $offset })
ui-rx-count-hint = Arrêter après ce nombre de trames correspondantes.
ui-rx-keep = Trames à conserver
ui-rx-keep-hint = Les N premières trames correspondantes sont conservées pour l'affichage (10000 au plus).
ui-rx-keep-max = 10000 au plus.
ui-rx-save = Enregistrer en pcapng (nom de fichier)
ui-rx-save-hint = Facultatif : les trames correspondantes sont aussi écrites dans ce fichier, dans le répertoire de capture du serveur.
ui-rx-save-same = Le fichier de sortie doit être différent du fichier d'entrée.
ui-rx-name-placeholder = Par défaut, backend:interface
ui-rx-options = Options
ui-rx-verify = Vérifier les signatures (pertes, ordre, latence)
ui-rx-promisc = Mode promiscuité
ui-rx-outgoing = Inclure les trames émises par cet hôte
ui-rx-start = Démarrer la capture
ui-rx-started = Capture « { $id } » démarrée.
ui-rx-list = Captures
ui-rx-empty = Aucune capture pour l'instant.
ui-rx-source = Interface / fichier
ui-rx-actions = Actions
ui-rx-stop = Arrêter
ui-rx-stop-named = Arrêter « { $id } »
ui-rx-delete-named = Supprimer « { $id } »
ui-rx-deleted = Capture supprimée.
ui-rx-delete-title = Supprimer cette capture ?
ui-rx-delete-body = Une capture en cours est d'abord arrêtée. Un fichier pcapng enregistré reste sur le serveur.
ui-rx-download = Télécharger le pcapng
ui-rx-counters = Compteurs
ui-rx-matched = Correspondantes
ui-rx-matched-pps = Paquets/s correspondants
ui-rx-matched-bps = Débit correspondant
ui-rx-matched-bytes = Octets correspondants
ui-rx-saved = Enregistrées dans le fichier
ui-rx-kept = Conservées pour l'affichage
ui-rx-verify-title = Vérification
ui-rx-verify-off = La vérification des signatures est désactivée pour cette capture.
ui-rx-verify-none = Aucune trame signée reçue pour l'instant.
ui-rx-signed = Trames signées
ui-rx-unsigned = Trames non signées
ui-rx-lost = Perdues
ui-rx-stream = Flux
ui-rx-received = Reçues
ui-rx-seq-range = Numéros de séquence
ui-rx-out-of-order = Hors séquence
ui-rx-duplicates = Doublons
ui-rx-latency = Latence (min / moy / max)
ui-rx-jitter = Gigue
ui-rx-latency-note = Les trames perdues sont estimées à partir des trous dans les numéros de séquence. La latence n'a de sens que si l'émetteur et le récepteur partagent une horloge (même hôte, ou synchronisation PTP / NTP).
ui-rx-frames = Trames capturées
ui-rx-frames-empty = Aucune trame correspondante pour l'instant.
ui-rx-time = Temps (s)
ui-rx-length = Longueur
ui-rx-signature = Signature
ui-rx-sig-value = flux { $stream } · seq { $seq }
ui-rx-show-more = Afficher plus ({ $count } sur { $max })
ui-rx-frame-title = Trame { $index }
ui-rx-select-frame = Sélectionnez une trame pour voir ses octets et ses couches décodées.
ui-rx-decode-failed = La trame ne peut pas être décodée : { $error }
ui-rx-truncated = Seuls { $bytes } octets sur { $len } ont été capturés.

# --- P1 : descriptions des champs des protocoles L2 / d'encapsulation ---
lldp-tlvs = TLV LLDP (en-tête : type sur 7 bits + longueur sur 9 bits).
cdp-version = Version du protocole CDP (2).
cdp-ttl = Durée de validité de cette annonce, en secondes.
cdp-checksum = Somme de contrôle Internet sur l'en-tête CDP et les TLV (Auto).
cdp-tlvs = TLV CDP (type sur 16 bits, longueur sur 16 bits en-tête compris).
stp-protocol-id = Identifiant du protocole Spanning Tree (toujours 0).
stp-version = Version du protocole (0 = STP, 2 = RSTP, 3 = MSTP).
stp-type = Type de BPDU (0 = configuration, 2 = RST/MST, 0x80 = TCN).
stp-flags = Indicateurs de la BPDU (topology change, proposal, rôle, learning, forwarding, agreement, TCA).
stp-root-priority = Priorité du pont racine et extension system-id.
stp-root-mac = Adresse MAC du pont racine.
stp-root-path-cost = Coût du chemin vers le pont racine.
stp-bridge-priority = Priorité du pont émetteur et extension system-id.
stp-bridge-mac = Adresse MAC du pont émetteur.
stp-port-id = Identifiant du port émetteur (priorité + numéro).
stp-message-age = Âge de la BPDU, en 1/256 de seconde.
stp-max-age = Âge maximal de la BPDU avant expiration, en 1/256 de seconde.
stp-hello-time = Intervalle entre les BPDU de configuration, en 1/256 de seconde.
stp-forward-delay = Temporisation de transmission (forward delay), en 1/256 de seconde.
stp-trailer = Extension RSTP / MSTP (longueur version-1, longueur v3, enregistrements MSTI) ; octets opaques.
lacp-subtype = Sous-type Slow Protocols (1 = LACP).
lacp-actor-system = Adresse MAC du système Actor.
lacp-actor-state = Indicateurs d'état de l'Actor (activity, timeout, aggregation, sync, collecting, distributing, defaulted, expired).
lacp-partner-system = Adresse MAC du système Partner.
lacp-partner-state = Indicateurs d'état du Partner.
marker-subtype = Sous-type Slow Protocols (2 = Marker).
eapol-version = Version du protocole EAPOL.
eapol-type = Type de paquet EAPOL (0 = EAP, 1 = Start, 2 = Logoff, 3 = Key).
eapol-length = Longueur du corps EAPOL, en octets (Auto).
eapol-body = Corps EAPOL (le paquet EAP pour le type 0) ; octets opaques.
pppoe-version = Version du protocole PPPoE (1).
pppoe-type = Type PPPoE (1).
pppoe-code = Code PPPoE (PADI / PADO / PADR / PADS / PADT, 0 = données de session).
pppoe-session-id = Identifiant de session PPPoE.
pppoe-length = Longueur de la charge utile PPPoE, en octets (Auto).
pppoe-tags = TAG de découverte PPPoE (type sur 16 bits, longueur sur 16 bits).
ppp-protocol = Identifiant de protocole PPP de la charge utile (LCP / IPCP / IP / IPv6 / ...).
ppp-cp-code = Code du protocole de contrôle PPP (Configure-Request / Ack / Nak / ...).
ppp-cp-identifier = Identifiant de requête du protocole de contrôle PPP.
ppp-cp-length = Longueur du paquet du protocole de contrôle PPP, en octets (Auto).
ppp-cp-options = Options du protocole de contrôle PPP (TLV type / longueur sur 8 bits).
vxlan-flags = Indicateurs VXLAN (bit I = VNI valide).
vxlan-vni = VXLAN Network Identifier sur 24 bits.
gre-flags = Indicateurs et version GRE (bits de présence checksum / key / sequence).
gre-protocol-type = EtherType de la charge utile encapsulée.
nvgre-vsid = Virtual Subnet ID NVGRE sur 24 bits.
nvgre-flowid = FlowID NVGRE (entropie pour la répartition de charge).
mpls-label = Valeur de label MPLS sur 20 bits. 0-15 sont réservés (0 / 2 explicit null, 1 router alert, 3 implicit null, 7 entropy label indicator, 13 GAL, 14 OAM alert, 15 extension) ; les SID SR-MPLS valent base SRGB + index de SID.
mpls-tc = Classe de trafic sur 3 bits (QoS / ECN).
mpls-s = Bit de bas de pile (1 = dernier label ; Auto d'après la couche suivante).
mpls-ttl = Durée de vie (TTL) MPLS.
gtpu-version = Version GTP (1).
gtpu-pt = Type de protocole (1 = GTP, 0 = GTP').
gtpu-e = Indicateur de présence d'un en-tête d'extension.
gtpu-s = Indicateur de présence du numéro de séquence.
gtpu-pn = Indicateur de présence du numéro N-PDU.
gtpu-message-type = Type de message GTP-U (255 = G-PDU).
gtpu-length = Longueur de la charge utile après l'en-tête obligatoire, en octets (Auto).
gtpu-teid = Tunnel Endpoint Identifier.
geneve-version = Version Geneve (0).
geneve-opt-len = Longueur de la zone d'options, en mots de 4 octets.
geneve-o = Indicateur de paquet OAM.
geneve-c = Indicateur de présence d'options critiques.
geneve-protocol-type = EtherType de la charge utile encapsulée.
geneve-vni = Virtual Network Identifier Geneve sur 24 bits.
geneve-options = Options Geneve (opaques ; taille donnée par opt_len).
l2tp-flags = Indicateurs et version L2TP (bits type, length, sequence, offset, priority).
l2tp-body = Corps L2TP (champs conditionnels + charge utile) ; octets opaques.
l2tpv3-session-id = Session ID L2TPv3.

# --- éditeur de paquets (M4 phase 2) ---
ui-bld-palette = Protocoles
ui-bld-search = Rechercher un protocole
ui-bld-favorite = Favori
ui-bld-favorites = Favoris
ui-bld-layer-l2 = Liaison de données (L2)
ui-bld-layer-l3 = Réseau (L3)
ui-bld-layer-l4 = Transport (L4)
ui-bld-layer-l7 = Application (L7)
ui-bld-layer-payload = Charge utile
ui-bld-stack = Couches
ui-bld-empty = Faites glisser un protocole ici, ou ajoutez-en un depuis la palette.
ui-bld-unknown = inconnu
ui-bld-dup = Dupliquer la couche
ui-bld-suggest = Ajouter la couche suivante
ui-bld-binding-warn = Non annoncée par la couche inférieure
ui-bld-fix = Corriger
ui-bld-added = { $id } ajouté.
ui-bld-moved = Déplacée en position { $index }.
ui-bld-removed = Couche supprimée.
ui-bld-fields = Éditeur de champs
ui-bld-select-layer = Sélectionnez une couche pour modifier ses champs.
ui-bld-auto = Auto
ui-bld-manual = Manuel
ui-bld-gen = Gén.
ui-bld-computed = calculé à la construction
ui-bld-nsap-parts = AFI { $afi } · zone { $area } · ID système { $sys } · NSEL { $nsel }
ui-bld-estimate = Paquets du flux et leur taille totale
ui-bld-expands = se développe en { $count } (~{ $bytes })
ui-bld-rate = Débit
ui-bld-rate-hint = Débit d'émission enregistré avec le modèle (1000pps, 1.5Mpps, 10Gbps, ifg=10us)
ui-bld-duration = ~{ $time } d'émission
ui-bld-duration-unknown = durée d'émission : définissez un débit
ui-bld-duration-hint = Paquets à émettre (tx.count, sinon la longueur du flux) à ce débit, plafonné par tx.duration_secs
ui-bld-gen-kind = Type
ui-bld-gen-count = nombre :
ui-bld-gen-shorthand = Plage / liste / CIDR
ui-bld-gen-random = Aléatoire
ui-bld-gen-random-mac = MAC aléatoire
ui-bld-gen-counter = Compteur
ui-bld-gen-file = Fichier
ui-bld-gen-expr = Expression
ui-bld-hex = Hex
ui-bld-tree = Arbre
ui-bld-no-bytes = Pas encore de paquet.
ui-bld-name = Nom du modèle
ui-bld-need-name = Donnez d'abord un nom au modèle.
ui-bld-unsaved = Modifications non enregistrées
ui-bld-save-as = Enregistrer sous…
ui-bld-save-first = Enregistrez le modèle avant de créer une tâche.
ui-bld-recompute = Recalculer
ui-bld-auto-recompute = Recalculer l'aperçu en direct à chaque modification
ui-bld-index = Index dans le flux
ui-bld-index-prev = Paquet précédent
ui-bld-index-next = Paquet suivant
ui-bld-undo = Annuler
ui-bld-redo = Rétablir
ui-bld-parity-ok = L'aperçu dans le navigateur correspond au rendu du serveur.
ui-bld-parity-ok-short = identique
ui-bld-parity-bad = L'aperçu dans le navigateur ne correspond pas au rendu du serveur.
ui-bld-parity-bad-short = différent
ui-bld-parity-pending = Vérification du rendu du serveur…
ui-bld-help-title = Raccourcis de l'éditeur
ui-bld-help-add = Ajouter le protocole ayant le focus
ui-bld-help-move = Déplacer la couche sélectionnée
ui-bld-help-del = Supprimer la couche sélectionnée
ui-bld-help-undo = Annuler
ui-bld-help-redo = Rétablir
ui-bld-help-hex = Saisir de l'hexadécimal sur l'octet du curseur
ui-bld-help-caret = Déplacer le curseur hexadécimal
ui-bld-suggest-prev = Ajouter une couche au-dessus

# --- P1 : descriptions des champs des protocoles de redondance / multicast ---
vrrp-version = Version VRRP (2 = RFC 3768, 3 = RFC 5798).
vrrp-type = Type de paquet VRRP (1 = Advertisement).
vrrp-vrid = Virtual Router Identifier.
vrrp-priority = Priorité du routeur émetteur (255 = propriétaire de l'adresse, 0 = arrêt de la participation).
vrrp-count = Nombre d'adresses virtuelles dans l'annonce (Auto).
vrrp-auth-type = Type d'authentification VRRPv2 (0 = aucune, 1 = texte simple, 2 = IP AH).
vrrp-adver-int = Intervalle d'annonce VRRPv2, en secondes.
vrrp-max-adver-int = Intervalle d'annonce maximal VRRPv3, en centisecondes.
vrrp-checksum = Somme de contrôle Internet sur le message VRRP (VRRPv3 inclut le pseudo-en-tête IP) (Auto).
vrrp-addresses = Adresses du routeur virtuel.
vrrp-auth-data = Données d'authentification VRRPv2 (8 octets).
hsrp-version = Version HSRP (0 pour HSRPv1).
hsrp-op-code = Opération HSRP (0 = Hello, 1 = Coup, 2 = Resign, 3 = Advertise).
hsrp-state = État de l'émetteur (1 = Learn, 2 = Listen, 4 = Speak, 8 = Standby, 16 = Active).
hsrp-hello-time = Intervalle entre les messages Hello, en secondes.
hsrp-hold-time = Durée de validité d'un Hello, en secondes.
hsrp-priority = Priorité du routeur au sein du groupe.
hsrp-group = Numéro du groupe standby.
hsrp-auth-data = Mot de passe d'authentification en clair (8 octets, "cisco" par défaut).
hsrp-virtual-ip = Adresse IPv4 virtuelle du groupe.
hsrpv2-tlvs = TLV HSRPv2 (Group State, Interface State, authentification Text / MD5 ; type et longueur sur 8 bits).
glbp-version = Version GLBP (1).
glbp-group = Numéro de groupe GLBP.
glbp-owner-id = Adresse MAC du routeur émetteur.
glbp-tlvs = TLV GLBP (Hello, Request/Response, Authentication ; la longueur inclut l'en-tête).
igmp-type = Type de message IGMP (Auto d'après le corps : 0x11 query, 0x12 / 0x16 report, 0x17 leave, 0x22 report v3).
igmp-max-resp-code = Temps / code de réponse maximal en 1/10 s (queries ; zéro dans les reports).
igmp-checksum = Somme de contrôle Internet sur l'ensemble du message IGMP (Auto).
igmp-group = Adresse du groupe multicast (0.0.0.0 dans une general query).
igmp-s-flag = Indicateur de suppression du traitement côté routeur.
igmp-qrv = Querier's Robustness Variable.
igmp-qqic = Querier's Query Interval Code.
igmp-num-sources = Nombre d'adresses source.
igmp-sources = Adresses source.
igmp-num-records = Nombre d'enregistrements de groupe.
igmp-records = Enregistrements de groupe (type d'enregistrement, longueur des données aux., sources, adresse multicast).
mld-max-resp = Délai / code de réponse maximal, en millisecondes.
mld-address = Adresse multicast IPv6 (:: dans une general query).
pim-version = Version PIM (2).
pim-type = Type de message PIM (Auto d'après le corps : 0 Hello, 1 Register, 3 Join/Prune).
pim-checksum = Somme de contrôle Internet (message entier ; Register : 8 premiers octets seulement) (Auto).
pim-hello-options = Options Hello PIM (Holdtime, LAN Prune Delay, DR Priority, Generation ID, Address List).
pim-addr-family = Famille d'adresses de l'adresse encodée (1 = IPv4).
pim-upstream-neighbor = Adresse du voisin amont auquel le Join/Prune est adressé.
pim-num-groups = Nombre d'ensembles de groupes.
pim-holdtime = Durée de conservation de l'état Join/Prune, en secondes.
pim-groups = Ensembles de groupes (groupe encodé, sources encodées jointes et élaguées).
pim-register-border = Bit Border (l'émetteur est un PIM Multicast Border Router).
pim-register-null = Bit Null-Register (aucun paquet de données encapsulé).

# --- P1 : descriptions des champs des protocoles applicatifs ---
dhcp-op = Code d'opération du message BOOTP (1 = BOOTREQUEST, 2 = BOOTREPLY).
dhcp-htype = Type d'adresse matérielle (1 = Ethernet).
dhcp-hlen = Longueur de l'adresse matérielle, en octets (6 pour Ethernet).
dhcp-hops = Nombre de sauts de relais, incrémenté par chaque agent relais.
dhcp-xid = ID de transaction choisi par le client.
dhcp-secs = Secondes écoulées depuis le début de l'acquisition par le client.
dhcp-flags = Indicateurs BOOTP (bit 15 = réponse en broadcast demandée).
dhcp-ciaddr = Adresse IP du client (s'il en a déjà une).
dhcp-yiaddr = Adresse IP « Your » (client) attribuée par le serveur.
dhcp-siaddr = Adresse IP du serveur suivant (serveur de démarrage).
dhcp-giaddr = Adresse IP de l'agent relais (passerelle) ; non nulle en cas de relais.
dhcp-chaddr = Adresse matérielle (MAC) du client ; suivie de 10 octets de bourrage.
dhcp-sname = Nom d'hôte facultatif du serveur (64 octets, complété par des NUL).
dhcp-file = Nom du fichier de démarrage (128 octets, complété par des NUL).
dhcp-magic-cookie = Magic cookie DHCP (0x63825363).
dhcp-options = Options DHCP (code / longueur sur 8 bits ; Pad, End, Message Type, Relay Agent Information, ...).
dhcpv6-msg-type = Type de message DHCPv6 (Solicit, Advertise, Request, Reply, Relay-Forward, ...).
dhcpv6-transaction-id = ID de transaction sur 24 bits.
dhcpv6-options = Options DHCPv6 (code / longueur sur 16 bits ; Client ID, IA_NA, Relay Message, ...).
dhcpv6-hop-count = Nombre d'agents relais ayant relayé ce message.
dhcpv6-link-address = Adresse identifiant le lien du client (relais).
dhcpv6-peer-address = Adresse du client ou du relais d'où provient le message.
dns-id = Identifiant de requête, recopié dans la réponse.
dns-qr = Requête (0) ou réponse (1).
dns-opcode = Type de requête (0 = requête standard, 4 = NOTIFY, 5 = UPDATE).
dns-aa = Indicateur de réponse faisant autorité.
dns-tc = Indicateur de troncature.
dns-rd = Indicateur de récursion souhaitée.
dns-ra = Indicateur de récursion disponible.
dns-z = Bit réservé (doit valoir 0).
dns-ad = Indicateur de données authentifiées (DNSSEC).
dns-cd = Indicateur de vérification désactivée (DNSSEC).
dns-rcode = Code de réponse (0 = NOERROR, 3 = NXDOMAIN, ...).
dns-qdcount = Nombre d'entrées dans la section question.
dns-ancount = Nombre d'enregistrements de ressource dans la section réponse.
dns-nscount = Nombre d'enregistrements de ressource dans la section autorité.
dns-arcount = Nombre d'enregistrements de ressource dans la section additionnelle.
dns-sections = Sections question / réponse / autorité / additionnelle (noms en labels préfixés par leur longueur) ; octets opaques.
ntp-li = Indicateur de seconde intercalaire (3 = horloge non synchronisée).
ntp-version = Numéro de version NTP (4).
ntp-mode = Mode d'association (3 = client, 4 = serveur, 5 = broadcast).
ntp-stratum = Stratum (0 = non spécifié, 1 = référence primaire, 2-15 = secondaire).
ntp-poll = Intervalle d'interrogation maximal, en log2 secondes (signé).
ntp-precision = Précision de l'horloge, en log2 secondes (signé).
ntp-root-delay = Délai aller-retour vers l'horloge de référence (secondes en virgule fixe 16.16).
ntp-root-dispersion = Dispersion par rapport à l'horloge de référence (secondes en virgule fixe 16.16).
ntp-reference-id = ID de référence (code ASCII pour le stratum 1, sinon une adresse IPv4 ou un hachage).
ntp-reference-ts = Instant du dernier réglage de l'horloge système (horodatage NTP sur 64 bits).
ntp-origin-ts = Heure d'émission du client renvoyée par le serveur (horodatage NTP sur 64 bits).
ntp-receive-ts = Heure d'arrivée de la requête au serveur (horodatage NTP sur 64 bits).
ntp-transmit-ts = Heure de départ du paquet de l'émetteur (horodatage NTP sur 64 bits).
ntp-extensions = Champs d'extension et MAC ; octets opaques.
snmp-tag = Octet de tag BER (0x30 SEQUENCE, 0x02 INTEGER, 0x04 OCTET STRING).
snmp-msg-length = Longueur BER de la SEQUENCE du message SNMP (Auto ; forme courte, inférieure à 128).
snmp-version-length = Longueur BER de l'INTEGER de version (1).
snmp-version = Version SNMP (0 = v1, 1 = v2c).
snmp-community-length = Longueur BER de la chaîne de communauté, en octets.
snmp-community = Chaîne de communauté (p. ex. "public").
snmp-pdu-type = Tag de la PDU (0xa0 GetRequest, 0xa2 Response, 0xa4 Trap v1, 0xa7 SNMPv2-Trap, ...).
snmp-pdu-length = Longueur BER du contenu de la PDU (Auto ; forme courte, inférieure à 128).
snmp-pdu-body = Contenu de la PDU en TLV BER : request-id, error-status, error-index et la SEQUENCE variable-bindings.
snmp-trap-body = Contenu du trap en TLV BER : enterprise, agent-addr, generic-trap, specific-trap, time-stamp et variable bindings.
syslog-message = Texte du message Syslog, commençant par <PRI> (facility * 8 + severity).
radius-code = Type de paquet RADIUS (Access-Request, Access-Accept, Accounting-Request, ...).
radius-identifier = Identifiant associant requêtes et réponses.
radius-length = Longueur du paquet RADIUS entier, en octets (Auto).
radius-authenticator = Request / Response Authenticator sur 16 octets (Manuel).
radius-attributes = Attributs RADIUS (type sur 8 bits, longueur sur 8 bits en-tête compris).
tftp-opcode = Opcode TFTP (1 = RRQ, 2 = WRQ, 3 = DATA, 4 = ACK, 5 = ERROR, 6 = OACK).
tftp-filename = Nom de fichier (terminé par NUL).
tftp-mode = Mode de transfert : netascii, octet ou mail (terminé par NUL).
tftp-options = Paires de chaînes nom / valeur d'option (RFC 2347) : blksize, timeout, tsize, windowsize, ...
tftp-block = Numéro de bloc (à partir de 1).
tftp-data = Octets de données (512 par bloc ; un bloc plus court termine le transfert).
tftp-error-code = Code d'erreur TFTP (1 = fichier introuvable, 2 = violation d'accès, ...).
tftp-error-message = Message d'erreur (terminé par NUL).

# --- P1 : descriptions des champs des protocoles de routage ---
ospf-version = Version OSPF (2 = OSPFv2, 3 = OSPFv3).
ospf-type = Type de paquet OSPF (Hello / DB Description / LS Request / LS Update / LS Ack ; Auto d'après la couche suivante).
ospf-packet-length = Longueur du paquet OSPF en-tête compris, en octets (Auto).
ospf-router-id = Router ID du routeur émetteur.
ospf-area-id = Zone à laquelle appartient le paquet (0.0.0.0 = backbone).
ospf-checksum = Somme de contrôle Internet sur le paquet OSPF hors champ d'authentification (Auto).
ospf-au-type = Type d'authentification (0 = aucune, 1 = mot de passe simple, 2 = cryptographique).
ospf-authentication = Champ d'authentification sur 64 bits (mot de passe, ou key id / séquence pour l'authentification cryptographique).
ospf-network-mask = Masque de réseau de l'interface ou du réseau annoncé.
ospf-hello-interval = Intervalle entre les paquets Hello, en secondes.
ospf-options = Capacités optionnelles (bits E, MC, N/P, L, DC, O, DN).
ospf-router-priority = Priorité du routeur pour l'élection du DR (0 = jamais DR).
ospf-router-dead-interval = Secondes sans Hello avant que le voisin soit déclaré hors service.
ospf-designated-router = Designated Router (0.0.0.0 = aucun).
ospf-backup-designated-router = Backup Designated Router (0.0.0.0 = aucun).
ospf-neighbors = Router ID des voisins vus sur le lien.
ospf-interface-mtu = Plus grand datagramme IP que l'interface peut émettre sans fragmentation.
ospf-dbd-flags = Indicateurs Database Description (I = init, M = more, MS = master).
ospf-dd-sequence = Numéro de séquence Database Description.
ospf-lsa-headers = En-têtes de LSA (20 octets chacun ; sommes de contrôle et longueurs en Manuel).
ospf-lsr-requests = LSA demandées (LS type, Link State ID, routeur annonceur).
ospf-num-lsas = Nombre de LSA dans la mise à jour.
ospf-ls-age = Temps écoulé depuis la création de la LSA, en secondes (laissez 0 pour une somme de contrôle Auto exacte).
ospf-ls-type = Type de LSA (Auto d'après la couche du corps de LSA ; 9 / 10 / 11 = LSA opaque).
ospf-link-state-id = Link State ID identifiant la partie du réseau décrite.
ospf-advertising-router = Router ID du routeur à l'origine de la LSA.
ospf-ls-sequence = Numéro de séquence de la LSA (0x80000001 = initial).
ospf-ls-checksum = Somme de contrôle de Fletcher de la LSA hors LS age (Auto).
ospf-lsa-length = Longueur de la LSA en-tête compris, en octets (Auto).
ospf-router-lsa-flags = Indicateurs Router-LSA (V = extrémité de lien virtuel, E = ASBR, B = ABR).
ospf-num-links = Nombre de liens de routeur décrits.
ospf-router-links = Liens de routeur (link ID, link data, type, nombre de TOS, métrique).
ospf-attached-routers = Router ID des routeurs rattachés au réseau.
ospf-metric = Coût de la route.
ospf-external-e = Type de métrique externe (1 = type 2, 0 = type 1).
ospf-forwarding-address = Adresse vers laquelle le trafic à destination est transmis (0.0.0.0 = l'ASBR).
ospf-external-route-tag = Tag sur 32 bits transporté avec la route externe.
ospf-opaque-ls-type = Portée d'inondation de la LSA opaque (9 = lien local, 10 = zone, 11 = AS).
ospf-opaque-type = Type opaque (1 = TE, 4 = Router Information, 7 = Extended Prefix, 8 = Extended Link ; Auto d'après la couche du corps).
ospf-opaque-id = Opaque ID distinguant les LSA d'un même type opaque (24 bits).
ospf-opaque-tlvs = TLV de la LSA opaque (type et longueur sur 16 bits, valeurs complétées à 4 octets ; longueurs Auto).
ospfv3-checksum = Somme de contrôle Internet sur le paquet OSPFv3 avec le pseudo-en-tête IPv6 (Auto).
ospfv3-instance-id = Instance ID (sépare les instances OSPFv3 sur un même lien).
ospfv3-interface-id = Interface ID de l'interface émettrice.
ospfv3-options = Options OSPFv3 (bits V6, E, N, R, DC, AF).
ospfv3-ls-type = Code de fonction de la LSA avec les bits de portée U / S (Auto d'après la couche du corps de LSA).
ospfv3-router-links = Interfaces du routeur (type, métrique, interface ID, router ID du voisin).
ospfv3-prefix-length = Longueur du préfixe en bits.
ospfv3-prefix-options = Options de préfixe (bits NU, LA, P, DN).
ospfv3-address-prefix = Préfixe d'adresse, complété à un multiple de 4 octets.
ospfv3-external-flags = Indicateurs AS-External-LSA (E = métrique de type 2, F = forwarding address présente, T = route tag présent).
ospfv3-referenced-ls-type = LS type de la LSA référencée.
ospfv3-link-local-address = Adresse lien local de l'interface.
ospfv3-num-prefixes = Nombre de préfixes qui suivent.
ospfv3-prefixes = Entrées de préfixe (longueur, options, métrique / réservé, préfixe d'adresse) ; octets opaques.
ospfv3-referenced-link-state-id = Link State ID de la LSA référencée.
ospfv3-referenced-advertising-router = Routeur annonceur de la LSA référencée.
bgp-marker = Marqueur sur 16 octets (tous les bits à 1).
bgp-length = Longueur du message BGP en-tête compris, en octets (Auto).
bgp-type = Type de message BGP (OPEN / UPDATE / NOTIFICATION / KEEPALIVE / ROUTE-REFRESH ; Auto d'après la couche suivante).
bgp-version = Version BGP (4).
bgp-my-as = Numéro de système autonome de l'émetteur (23456 = AS_TRANS pour les AS sur 4 octets).
bgp-hold-time = Hold time proposé, en secondes (0 ou au moins 3).
bgp-identifier = Identifiant BGP (router ID) de l'émetteur.
bgp-opt-params-length = Longueur des paramètres optionnels, en octets (Auto).
bgp-opt-params = Paramètres optionnels (type 2 = capacités).
bgp-withdrawn-routes-length = Longueur des routes retirées, en octets (Auto).
bgp-withdrawn-routes = Routes IPv4 retirées (longueur du préfixe en bits, préfixe).
bgp-total-path-attribute-length = Longueur des attributs de chemin, en octets (Auto).
bgp-path-attributes = Attributs de chemin (indicateurs + type code ; la longueur passe sur 16 bits avec l'indicateur Extended Length, positionné automatiquement au-delà de 255 octets).
bgp-nlri = Routes IPv4 annoncées (longueur du préfixe en bits, préfixe) ; les autres familles vont dans MP_REACH_NLRI.
bgp-error-code = Code d'erreur de la NOTIFICATION.
bgp-error-subcode = Sous-code d'erreur de la NOTIFICATION.
bgp-error-data = Données de diagnostic de la NOTIFICATION.
bgp-afi = Address Family Identifier (1 = IPv4, 2 = IPv6).
bgp-rr-subtype = Sous-type ROUTE-REFRESH (0 = normal, 1 = BoRR, 2 = EoRR).
bgp-safi = Subsequent Address Family Identifier (1 = unicast, 2 = multicast).
rip-command = Commande RIP (1 = requête, 2 = réponse).
rip-version = Version RIP (1 ou 2 ; RIPng utilise 1).
rip-entries = Entrées de route (AFI, route tag, adresse, masque, next hop, métrique).
ripng-entries = Entrées de la table de routage (préfixe, route tag, longueur du préfixe, métrique ; 0xff = next hop).
eigrp-version = Version de l'en-tête EIGRP (2).
eigrp-opcode = Opcode EIGRP (Update / Query / Reply / Hello / SIA-Query / SIA-Reply).
eigrp-checksum = Somme de contrôle Internet sur le paquet EIGRP (Auto).
eigrp-flags = Indicateurs EIGRP (Init, Conditional Receive, Restart, End of Table).
eigrp-sequence = Numéro de séquence pour la livraison fiable.
eigrp-acknowledge = Numéro de séquence acquitté (0 = aucun).
eigrp-virtual-router-id = ID de routeur virtuel (0 = famille d'adresses unicast).
eigrp-autonomous-system = Numéro de système autonome EIGRP.
eigrp-tlvs = TLV EIGRP (type sur 16 bits, longueur sur 16 bits en-tête compris).

# --- P2 : descriptions des champs de l'encapsulation OSI ---
osi-nlpid = Identifiant de protocole de couche réseau (ISO/IEC TR 9577) : 0x81 CLNP, 0x82 ES-IS, 0x83 IS-IS.
osi-data = Reste de la PDU OSI après le NLPID (octets opaques).
clnp-nlpid = Identifiant de protocole de couche réseau (0x81 = CLNP).
clnp-length-indicator = Longueur de l'en-tête en octets, jusqu'à la fin des options (Auto).
clnp-version = Version / extension de l'identifiant de protocole (1).
clnp-lifetime = Durée de vie restante, en unités de 500 ms.
clnp-sp = SP : segmentation autorisée ; la partie segmentation est présente (Auto d'après la partie segmentation).
clnp-ms = MS : d'autres segments suivent.
clnp-er = E/R : envoyer un rapport d'erreur si la PDU est rejetée.
clnp-pdu-type = Type de PDU : 0x1c DT (données), 0x1d MD (données multicast), 0x01 ER (rapport d'erreur), 0x1e ERQ / 0x1f ERP (requête / réponse echo).
clnp-segment-length = Longueur de la PDU entière, en-tête et données (Auto).
clnp-checksum = Somme de contrôle de Fletcher ISO 8473 de l'en-tête (Auto). 0 = somme de contrôle non utilisée.
clnp-destination = Adresse NSAP de destination.
clnp-source = Adresse NSAP source.
clnp-segmentation = Partie segmentation (identifiant d'unité de données, offset du segment, longueur totale), présente quand SP est positionné. La longueur totale n'est pas calculée.
clnp-options = Options (code sur 8 bits / longueur sur 8 bits) jusqu'à l'indicateur de longueur : sécurité, maintien de la QoS, priorité, routage par la source, enregistrement de la route, bourrage, motif de rejet.
esis-nlpid = Identifiant de protocole de couche réseau (0x82 = ES-IS).
esis-length-indicator = Longueur de la PDU en octets (Auto).
esis-version = Version / extension de l'identifiant de protocole (1).
esis-pdu-type = Type de PDU : 2 ESH (end system hello), 4 ISH (intermediate system hello), 6 RD (redirect).
esis-holding-time = Holding time en secondes.
esis-checksum = Somme de contrôle de Fletcher ISO 8473 de la PDU (Auto). 0 = somme de contrôle non utilisée.
esis-number-of-source-addresses = Nombre d'adresses source (Auto).
esis-source-addresses = Adresses NSAP desservies par le système terminal.
esis-net = Network entity title du système intermédiaire (redirect : vide si le prochain saut est un système terminal).
esis-destination = Adresse NSAP de destination redirigée.
esis-bsnpa-length = Longueur de l'adresse de sous-réseau (Auto).
esis-bsnpa = Adresse de sous-réseau (p. ex. adresse MAC) du meilleur prochain saut.
esis-options = Options (code sur 8 bits / longueur sur 8 bits) jusqu'à l'indicateur de longueur : sécurité, maintien de la QoS, priorité, masque d'adresse, masque SNPA, temporisateur de configuration ES suggéré.
esis-data = Reste de la PDU après la partie fixe (octets opaques).
isis-irpd = Discriminateur de protocole de routage intradomaine (0x83 = IS-IS).
isis-header-length = Longueur de l'en-tête fixe en octets (constante pour chaque type de PDU : LAN Hello / LSP 27, P2P Hello 20, CSNP 33, PSNP 17).
isis-version-ext = Version / extension de l'identifiant de protocole (1).
isis-id-length = Longueur du System ID (0 = 6 octets).
isis-pdu-type = Type de PDU (15/16 LAN Hello L1/L2, 17 P2P Hello, 18/20 LSP L1/L2, 24/25 CSNP, 26/27 PSNP).
isis-version = Version (1).
isis-max-area-addresses = Nombre maximal d'adresses de zone (0 = 3).
isis-pdu-length = Longueur de la PDU entière (Auto). Sur un Hello, une valeur Manuelle complète la PDU jusqu'à cette longueur avec des TLV Padding (p. ex. 1497 pour un MTU de 1500).
isis-tlvs = TLV (code sur 8 bits / longueur sur 8 bits) et leurs sous-TLV.
isis-source-id = System ID de l'émetteur.
isis-holding-time = Holding time en secondes.
isis-priority = Priorité pour l'élection de l'IS désigné (0–127).
isis-lan-id = LAN ID : System ID de l'IS désigné et ID de pseudo-nœud.
isis-circuit-type = Type de circuit : 1 = L1, 2 = L2, 3 = L1/L2.
isis-local-circuit-id = ID de circuit local.
isis-remaining-lifetime = Durée de vie restante du LSP en secondes (non couverte par la somme de contrôle).
isis-lsp-id = LSP ID : System ID, ID de pseudo-nœud et numéro de fragment.
isis-sequence-number = Numéro de séquence du LSP.
isis-lsp-checksum = Somme de contrôle de Fletcher du LSP ID jusqu'à la fin de la PDU (Auto).
isis-partition-repair = Bit P : réparation de partition prise en charge.
isis-attached = Bits ATT : rattaché à une autre zone (métrique default / delay / expense / error).
isis-overload = Bit OL : surcharge de la base de données LSP.
isis-is-type = Type d'IS : 1 = L1, 3 = L1/L2.
isis-snp-source-id = System ID de l'émetteur et ID de circuit (0).
isis-start-lsp-id = Premier LSP ID couvert par le CSNP.
isis-end-lsp-id = Dernier LSP ID couvert par le CSNP.
isis-data = Reste de la PDU après l'en-tête commun (octets opaques).

# --- P2 : descriptions des champs SCTP ---
sctp-src-port = Port source.
sctp-dst-port = Port de destination.
sctp-verification-tag = Verification tag : l'initiate tag du pair (0 dans un paquet portant INIT).
sctp-checksum = CRC32c de l'ensemble du paquet SCTP avec ce champ à zéro (Auto), envoyé octet de poids faible en premier.
sctp-chunks = Chunks (type sur 8 bits, indicateurs sur 8 bits, longueur sur 16 bits incluant l'en-tête de 4 octets, bourrage à zéro jusqu'à 4 octets) : DATA, INIT, INIT ACK, SACK, HEARTBEAT, ABORT, SHUTDOWN, ERROR, COOKIE ECHO / ACK, AUTH, I-DATA, ASCONF, RE-CONFIG, FORWARD TSN, PAD. Les longueurs, le bourrage et les compteurs SACK sont Auto.

# --- P2 : descriptions des champs LDP / RSVP-TE / BFD ---
ldp-version = Version LDP (1).
ldp-pdu-length = Longueur de la PDU après les champs version et longueur de PDU (Auto).
ldp-lsr-id = LSR ID : les quatre premiers octets de l'identifiant LDP.
ldp-label-space = ID d'espace de labels : les deux derniers octets de l'identifiant LDP (0 = global à la plateforme).
ldp-messages = Messages (type sur 16 bits avec le bit U, longueur de la valeur sur 16 bits, ID de message, TLV) : Notification, Hello, Initialization, KeepAlive, Address / Address Withdraw, Label Mapping / Request / Withdraw / Release / Abort Request. TLV : FEC, Address List, Hop Count, Path Vector, Generic Label, Status, Common Hello / Session Parameters, adresses de transport, Configuration Sequence Number. Les longueurs sont Auto.
rsvp-version = Version RSVP (1).
rsvp-flags = Indicateurs (bit 0 : capable de refresh reduction, RFC 2961).
rsvp-msg-type = Type de message (1 Path, 2 Resv, 3 PathErr, 4 ResvErr, 5 PathTear, 6 ResvTear, 7 ResvConf, 12 Bundle, 13 Ack, 15 Srefresh, 20 Hello).
rsvp-checksum = Somme de contrôle Internet du message entier avec ce champ à zéro (Auto).
rsvp-send-ttl = TTL IP avec lequel le message a été envoyé.
rsvp-reserved = Réservé (0).
rsvp-length = Longueur du message entier en octets, en-tête commun compris (Auto).
rsvp-objects = Objets (longueur sur 16 bits incluant l'en-tête de 4 octets, Class-Num, C-Type) : SESSION, RSVP_HOP, TIME_VALUES, ERROR_SPEC, SCOPE, STYLE, FLOWSPEC, FILTER_SPEC, SENDER_TEMPLATE, SENDER_TSPEC, RESV_CONFIRM, LABEL, LABEL_REQUEST, EXPLICIT_ROUTE, RECORD_ROUTE, HELLO, MESSAGE_ID, SESSION_ATTRIBUTE. Les longueurs, la longueur du nom de session et le bourrage sont Auto.
bfd-version = Version BFD (1).
bfd-diag = Code de diagnostic : motif du dernier changement d'état de la session locale.
bfd-state = État de la session : 0 AdminDown, 1 Down, 2 Init, 3 Up.
bfd-p = Bit P : Poll (changement de paramètre, le pair répond avec F).
bfd-f = Bit F : Final (réponse à un Poll).
bfd-c = Bit C : Control Plane Independent.
bfd-a = Bit A : authentification présente (Auto : positionné en présence d'une section d'authentification).
bfd-d = Bit D : mode Demand.
bfd-m = Bit M : Multipoint (0).
bfd-detect-mult = Multiplicateur du temps de détection.
bfd-length = Longueur du paquet de contrôle en octets, section d'authentification comprise (Auto).
bfd-my-discriminator = My discriminator : valeur non nulle propre à la session chez l'émetteur.
bfd-your-discriminator = Your discriminator : le discriminateur du pair (0 tant qu'il est inconnu).
bfd-desired-min-tx = Intervalle TX minimal souhaité, en microsecondes.
bfd-required-min-rx = Intervalle RX minimal requis, en microsecondes.
bfd-required-min-echo-rx = Intervalle RX echo minimal requis, en microsecondes (0 = pas d'echo).
bfd-authentication = Section d'authentification (type sur 8 bits, longueur sur 8 bits en-tête compris) : mot de passe simple, keyed MD5 / SHA1 et leurs variantes meticulous. Le condensé / hachage est Manuel.

# --- Documentation générée (`wpg docs gen`, crates/wpg-cli/src/cmd/docs) ---
docs-gen-no-root = { $path } n'est pas la racine du dépôt (pas de docs/en/src/SUMMARY.md) ; passez --root.
docs-gen-error = Échec de la génération de la documentation : { $error }
docs-gen-stale = obsolète : { $path }
docs-gen-extra = n'est plus généré : { $path }
docs-gen-up-to-date = La documentation générée est à jour.
docs-gen-run-hint = Exécutez `just docs-gen` (ou `wpg docs gen`) et committez le résultat.
docs-gen-done = Documentation générée : { $written } fichiers écrits, { $removed } supprimés.
docgen-yes = oui
docgen-octets = { $octets } octets
docgen-items = { $count } éléments
docgen-variant = Variante { $n }
docgen-col-id = ID
docgen-col-name = Nom
docgen-col-fields = Champs
docgen-col-next-by = Couche suivante selon
docgen-col-notes = Remarques
docgen-col-lower = Couche inférieure
docgen-col-selector = Sélecteur
docgen-col-value = Valeur
docgen-col-protocol = Protocole
docgen-col-note = Remarque
docgen-col-level = Niveau
docgen-col-rule = Règle
docgen-col-field = Champ
docgen-col-type = Type
docgen-col-default = Défaut
docgen-col-auto = Auto
docgen-col-description = Description
docgen-col-bit = Bit
docgen-col-mask = Masque
docgen-col-type-value = Type
docgen-col-value-fields = Champs de la valeur
docgen-col-key = Clé
docgen-col-required = Requis
docgen-col-in = Dans
docgen-col-status = Statut
docgen-col-body = Corps
docgen-col-tag = Tag
docgen-col-endpoint = Point de terminaison
docgen-col-summary = Résumé
docgen-col-env = Variable d'environnement
docgen-layer-l2 = L2 (liaison de données)
docgen-layer-l3 = L3 (réseau)
docgen-layer-l4 = L4 (transport)
docgen-layer-l7 = Application et contrôle
docgen-layer-payload = Charge utile
docgen-proto-title = Référence des protocoles
docgen-proto-intro = Ce chapitre est généré à partir de l'IR de définition des protocoles de `wpg-protocols` (le catalogue servi par `GET /api/protocols`) ; il correspond donc toujours au code : { $count } protocoles. Chaque page liste les champs dans l'ordre de transmission avec leur type, leur valeur par défaut, leur règle Auto et leur description, les couches inférieures qui mènent au protocole, la table des couches suivantes, les contraintes vérifiées après la construction et les modèles d'exemple fournis. Les [notes sur les protocoles](../protocols/overview.md), rédigées à la main, expliquent comment on atteint les protocoles, quels champs vous renseignez à la main et les approximations qui subsistent. Après avoir modifié une définition, régénérez avec `just docs-gen`.
docgen-proto-layer = Couche
docgen-proto-header = En-tête
docgen-proto-header-fixed = { $octets } octets (fixe)
docgen-proto-header-variable = longueur variable
docgen-proto-min-frame = Trame minimale
docgen-proto-synth = Générateur
docgen-proto-synth-payload = générateur de charge utile intégré (les champs ci-dessous sont des paramètres, pas des champs transmis)
docgen-proto-pseudo = Pseudo-en-tête
docgen-proto-pseudo-fields = fournit { $src } / { $dst } aux sommes de contrôle des couches supérieures
docgen-proto-notes = Remarques
docgen-proto-final-dst = Destination finale
docgen-proto-final-dst-text = tant que { $when } n'est pas 0, l'élément 0 de { $list } remplace l'adresse de destination du pseudo-en-tête inférieur (RFC 8200 §8.1)
docgen-proto-inbound = Atteint depuis
docgen-proto-inbound-none = Aucun protocole du catalogue ne mène ici : utilisez-le comme première couche ou empilez-le explicitement.
docgen-proto-fields = Champs
docgen-proto-params = Paramètres
docgen-proto-no-fields = Aucun champ.
docgen-proto-next = Couche suivante
docgen-proto-next-intro = La couche suivante est choisie par { $field } ; lorsque le champ est Auto, l'éditeur écrit la valeur du protocole empilé au-dessus.
docgen-proto-next-alt = Au décodage, { $field } est aussi consulté si le premier champ ne correspond à aucune entrée.
docgen-proto-constraints = Contraintes
docgen-proto-samples = Modèles d'exemple
docgen-proto-samples-none = Aucun modèle fourni n'utilise encore ce protocole.
docgen-proto-samples-more = et { $count } de plus
docgen-bind-default = (toute autre valeur)
docgen-bind-min-len = au moins { $octets } octets restants
docgen-bind-reparse = redécode cette couche
docgen-bind-peek = Les { $bits } premiers bits de la charge utile quand { $field } = { $value }
docgen-bind-peek-skip = { $bits } bits de la charge utile après ses { $skip } premiers bits quand { $field } = { $value }
docgen-level-error = erreur
docgen-level-warning = avertissement
docgen-level-info = info
docgen-c-range = { $field } dans { $min }..{ $max }
docgen-c-equals = { $field } = { $value }
docgen-c-not-equals = { $field } ≠ { $value }
docgen-c-min-length = { $span } d'au moins { $octets } octets
docgen-c-max-length = { $span } d'au plus { $octets } octets
docgen-span-header = en-tête
docgen-span-payload = charge utile
docgen-span-total = en-tête et charge utile
docgen-unit-bytes = octets
docgen-unit-words16 = mots de 16 bits
docgen-unit-words32 = mots de 32 bits
docgen-unit-words64 = mots de 64 bits
docgen-auto-length = Longueur de : { $span }, en { $unit }
docgen-auto-checksum = { $algorithm } sur { $covers }
docgen-auto-nonzero = un 0 calculé est envoyé avec tous les bits à 1
docgen-auto-exclude = { $fields } compté(s) comme zéro
docgen-auto-next = Numéro de protocole de la couche suivante (voir Couche suivante)
docgen-auto-count = Nombre d'éléments de { $field }
docgen-auto-size = Taille de { $field } en octets
docgen-auto-counter = Compteur par flux (la valeur par défaut est utilisée pour l'instant)
docgen-cksum-internet = Somme de contrôle Internet
docgen-cksum-internet-pseudo = Somme de contrôle Internet avec pseudo-en-tête
docgen-cksum-fletcher = Somme de contrôle de Fletcher
docgen-cksum-crc32c = CRC-32C
docgen-cksum-crc32 = CRC-32
docgen-cover-header = l'en-tête
docgen-cover-all = l'en-tête et tout ce qui suit
docgen-len-from-field = { $expr } octets
docgen-len-count = { $field } éléments
docgen-len-until = jusqu'à l'offset { $expr } du groupe
docgen-len-to-end = reste de la PDU
docgen-len-to-end-except = reste de la PDU sauf ses { $octets } derniers octets
docgen-len-until-terminator = jusqu'au terminateur { $octets } (ajouté à la construction)
docgen-len-until-tlv = jusqu'au premier élément de type { $type } inclus, ou jusqu'à la fin de la PDU
docgen-tlv-uncounted = Les { $octets } premiers octets de valeur après la longueur n'y sont pas comptés.
docgen-list-item = Chaque élément est un groupe des champs ci-dessous.
docgen-tlv-format = Type sur { $type } bits, longueur sur { $len } bits.
docgen-tlv-len-first = La longueur précède le type.
docgen-tlv-len-incl = La longueur compte aussi les champs type et longueur.
docgen-tlv-len-excl = La longueur ne compte que la valeur.
docgen-tlv-unit = Unité de longueur : { $unit }.
docgen-tlv-head = Les { $octets } premiers octets de valeur se trouvent entre le type et la longueur.
docgen-tlv-align = Chaque élément est complété par des zéros jusqu'à un multiple de { $align } octets (non compté dans la longueur)
docgen-tlv-unpadded-last = le dernier élément n'est pas complété
docgen-tlv-long = Le bit de type { $flag } sélectionne une longueur sur { $len } bits.
docgen-tlv-fill = Une valeur Manuelle de { $field } complète la zone avec des éléments { $variant } jusqu'à cette taille.
docgen-tlv-unknown-raw = Les types inconnus sont conservés en octets de valeur bruts.
docgen-tlv-unknown-stop = Un type inconnu met fin au décodage typé (le reste devient un seul élément brut).
docgen-tlv-type-only = (type seul, sans longueur ni valeur)
docgen-tlv-when-prefix = quand la valeur commence par { $octets }
docgen-tlv-when-len = quand la valeur fait { $octets } octets
docgen-notes-heading = Référence générée
docgen-notes-intro = Référence des champs des protocoles de cette page (générée à partir des définitions) :
docgen-cli-title = Référence de la CLI
docgen-cli-intro = Le binaire `wpg` exécute à la fois le serveur (`wpg serve`) et les outils sans interface, il convient donc aussi à la CI et aux sessions ssh. Par exemple, `wpg send tpl.wpg.yaml -i eth0 --rate 1Mpps` émet un modèle. Cette page est générée à partir des définitions des commandes : chaque bloc est la sortie `--help` exacte de la version actuelle. Les messages d'exécution suivent `LANG` / `LC_ALL` ; le texte de `--help` lui-même est en anglais.
docgen-cli-commands = Commandes
docgen-cli-note-send = Émet un modèle. `--plan` choisit entre le développement de toutes les trames à l'avance (`prerender`) et leur construction pendant l'émission (`onthefly`). `--json` affiche le rapport final sur stdout.
docgen-cli-note-render = Écrit les trames d'un modèle dans un fichier pcapng sans les émettre.
docgen-cli-note-import = Transforme une capture (ou de l'hexadécimal collé) en modèle ; voir le [tutoriel de rejeu pcap](../tutorials/pcap-replay.md).
docgen-cli-note-bench = Émission à débit illimité d'une trame synthétique (EtherType 0x88b5), ou d'un modèle avec `--template`. Voir `docs/perf.md` pour les valeurs de référence. Champs de la sortie `--json` : backend, detail, iface, frame_size, batch_size, packets, dropped, pps, bps (L2), l1_bps, elapsed.
docgen-cli-note-ifaces = Liste les interfaces avec MAC, adresses, MTU, état du lien et pilote.
docgen-cli-note-doctor = Exécute les vérifications de l'environnement décrites dans [Permissions](../getting-started/permissions.md) ; le code de sortie est non nul si une vérification échoue.
docgen-cli-note-template = Outils de modèles. `lint` se termine avec le code 1 en cas d'erreur ; `export` et `import` emballent les modèles dans des archives `.wpgbundle` et les en extraient.
docgen-cli-note-config = Outils pour `wpg.toml` ; voir la [référence de configuration](config.md).
docgen-api-title = Référence de l'API
docgen-api-intro = wpg-server fournit une API REST (JSON) sous `/api` et diffuse les statistiques en direct, les changements d'état des tâches et les journaux via un WebSocket (`GET /api/ws`). Cette page est générée à partir du document OpenAPI du serveur (utoipa). Un serveur en fonctionnement sert le même document à `/api/openapi.json` avec Swagger UI à `/api/docs/`, et une copie est conservée dans [`docs/schema/openapi.json`](https://github.com/SeiyaFunaokaJP/web-packet-generator/blob/main/docs/schema/openapi.json). Connectez-vous avec `POST /api/auth/login` (ou `POST /api/auth/setup` au premier lancement) ; le cookie `wpg_session` authentifie ensuite chaque requête, et les requêtes POST / PUT / DELETE doivent aussi envoyer le jeton CSRF de `GET /api/auth/status` dans l'en-tête `X-CSRF-Token`. Les erreurs sont un objet JSON `error` avec `code`, `message` (dans la langue de `Accept-Language`) et `details`. Les descriptions des points de terminaison et des schémas ci-dessous proviennent du code.
docgen-api-endpoints = Points de terminaison
docgen-api-params = Paramètres
docgen-api-body = Corps de la requête
docgen-api-responses = Réponses
docgen-api-schemas = Schémas
docgen-api-schemas-intro = Corps de requête et de réponse référencés ci-dessus.
docgen-api-tag-auth = Connexion, déconnexion et configuration initiale.
docgen-api-tag-system = État de santé, version, interfaces, autodiagnostic et backends d'émission.
docgen-api-tag-protocols = Le catalogue des protocoles (voir la [référence des protocoles](protocols.md)).
docgen-api-tag-packets = Rendu, décodage et vérification (lint) des paquets.
docgen-api-tag-templates = La bibliothèque de modèles.
docgen-api-tag-jobs = Tâches d'émission et statistiques.
docgen-api-tag-events = Le flux d'événements WebSocket.
docgen-config-title = Configuration
docgen-config-intro = Le serveur lit `wpg.toml` dans le répertoire de travail, ou le fichier indiqué par `--config` / `WPG_CONFIG`. Ordre de priorité, du plus faible au plus fort : valeurs par défaut intégrées, le fichier, les variables d'environnement `WPG_<SECTION>__<KEY>` (`__` sépare les niveaux d'imbrication, par exemple `WPG_SERVER__BIND=0.0.0.0:8080` ou `WPG_TX__ALLOWED_IFACES='["eth1"]'`), puis les options de ligne de commande telles que `wpg serve --bind`. Les clés inconnues sont des erreurs. `wpg config init` écrit l'exemple commenté ci-dessous, `wpg config show` affiche la configuration effective, `wpg config validate` la vérifie et `wpg config set-password` enregistre le hachage du mot de passe administrateur. Le mot de passe administrateur lui-même n'est pas une clé : `wpg serve --password`, `--password-file` et `WPG_PASSWORD` ont priorité sur `auth.password_hash` et `auth.password_file`. Cette page est générée à partir des types de configuration.
docgen-config-keys = Clés
docgen-config-top-level = Niveau supérieur
docgen-config-default-machine = Répertoire de données du système (Linux : `~/.local/share/wpg`)
docgen-config-example = Exemple (`wpg config init`)
docgen-config-example-intro = Chaque ligne active est égale à la valeur par défaut intégrée : le fichier ne change donc rien tant qu'il n'est pas modifié ; les lignes commentées montrent les clés optionnelles.
docgen-config-data-dir = Répertoire de l'état du serveur : modèles utilisateur, favoris, base de données SQLite (utilisateurs, sessions, historique des tâches), captures de simulation et certificat TLS auto-signé.
docgen-config-server = Écoute HTTP.
docgen-config-server-bind = Adresse d'écoute. Sans mot de passe (et sans utilisateur créé sur l'écran de configuration), seules les adresses de bouclage sont acceptées (`--allow-insecure` contourne cette restriction).
docgen-config-server-tls = Chaîne de certificats et clé privée TLS (PEM) pour servir en HTTPS ; prioritaire sur `self_signed_tls`.
docgen-config-server-tls-cert = Chaîne de certificats PEM.
docgen-config-server-tls-key = Clé privée PEM.
docgen-config-server-self-signed-tls = Servir en HTTPS avec un certificat auto-signé généré dans `<data_dir>/tls/` (réutilisé aux démarrages suivants) ; `wpg serve --tls` l'active.
docgen-config-server-base-url = URL publique du serveur derrière un reverse proxy (`https://wpg.example.net`), utilisée pour les liens absolus.
docgen-config-auth = Authentification.
docgen-config-auth-password-hash = Chaîne PHC argon2id du mot de passe administrateur ; `wpg config set-password` l'écrit.
docgen-config-auth-password-file = Fichier contenant le mot de passe administrateur : une chaîne PHC argon2, ou du texte en clair haché au démarrage.
docgen-config-auth-session-ttl-secs = Durée de vie de la session en secondes (glissante : chaque requête la prolonge).
docgen-config-templates = Bibliothèque de modèles.
docgen-config-templates-library-paths = Bibliothèques de modèles en lecture seule (dépôts Git clonés, par exemple), parcourues récursivement à la recherche de `*.wpg.yaml` / `*.wpg.json` et rechargées quand les fichiers changent.
docgen-config-templates-user-dir = Répertoire accessible en écriture pour les modèles créés ou importés dans l'interface graphique ; par défaut `<data_dir>/templates`.
docgen-config-protocols = Protocoles définis par l'utilisateur.
docgen-config-protocols-dirs = Répertoires de fichiers de protocoles définis par l'utilisateur (`*.yaml` : nouvelles définitions de protocoles et liaisons sous des protocoles existants), chargés au démarrage dans l'ordre des noms de fichiers. `wpg serve --protocols-dir` en ajoute d'autres.
docgen-config-tx = Politique et valeurs par défaut d'émission.
docgen-config-tx-allowed-ifaces = Interfaces sur lesquelles les tâches peuvent émettre ; vide = toutes. Le backend `null` et les simulations ne sont pas restreints.
docgen-config-tx-max-rate = Débit maximal qu'une tâche peut demander (`100kpps`, `1Gbps`) ; les tâches sans débit sont émises à ce débit.
docgen-config-tx-default-backend = Backend utilisé quand une tâche n'en indique aucun (`null`, `pcapfile`, `afpacket`, `pcap`, `afxdp`, `dpdk`) ; non défini = `afpacket` sous Linux, sinon `pcap`, sinon `null`.
docgen-config-tx-dpdk-eal-args = Arguments EAL DPDK pour le backend `dpdk`, p. ex. `["-a", "0000:00:04.0"]`. wpg ajoute `-l <premier CPU>` et `--in-memory` s'ils ne sont pas fournis. EAL démarre une seule fois par processus avec la première tâche `dpdk` ; les modifier nécessite un redémarrage.
docgen-config-log = Journalisation.
docgen-config-log-level = Directives de filtrage (`RUST_LOG` les remplace), par exemple `info` ou `wpg_tx=debug,info`.
docgen-config-log-format = Format des lignes : `pretty` (lisible) ou `json` (un objet par ligne).
docgen-config-log-file = Écrire aussi dans ce fichier (son répertoire est créé).
docgen-config-log-rotation = Rotation du fichier : `never`, `hourly` ou `daily`.
docgen-config-log-max-files = Conserver au plus ce nombre de fichiers après rotation (les plus anciens sont supprimés).
docgen-config-log-ansi = Coloriser la sortie console quand il s'agit d'un terminal.
docgen-schema-heading = Référence des clés (générée)
docgen-schema-intro = Généré à partir du JSON Schema du format de modèle (`wpg template schema`) ; les descriptions sont les commentaires de documentation du code (en anglais). `Template` est la racine du document.
# --- P2 : descriptions des champs PTP / ESMC / CFM / R-APS / OAM 802.3ah ---
ptp-major-sdo-id = majorSdoId (transportSpecific dans IEEE 1588-2008) : 0 pour les profils par défaut, 1 pour IEEE 802.1AS (gPTP).
ptp-message-type = Type de message : 0 Sync, 1 Delay_Req, 2 Pdelay_Req, 3 Pdelay_Resp, 8 Follow_Up, 9 Delay_Resp, 10 Pdelay_Resp_Follow_Up, 11 Announce, 12 Signaling, 13 Management.
ptp-minor-version = minorVersionPTP (0 dans IEEE 1588-2008, 1 dans IEEE 1588-2019).
ptp-version = versionPTP (2).
ptp-message-length = Longueur du message entier en octets, TLV compris (Auto).
ptp-domain-number = Numéro de domaine.
ptp-minor-sdo-id = minorSdoId (réservé, 0, dans IEEE 1588-2008).
ptp-flags = Champ d'indicateurs : alternateMaster, twoStep, unicast, les bits propres au profil et de sécurité dans le premier octet ; leap61, leap59, currentUtcOffsetValid, ptpTimescale, timeTraceable, frequencyTraceable et synchronizationUncertain dans le second.
ptp-correction-field = Correction en nanosecondes multipliée par 2^16 (signée).
ptp-message-type-specific = Champ propre au type de message (réservé, 0, dans IEEE 1588-2008).
ptp-clock-identity = Identité d'horloge du port source (EUI-64, p. ex. une adresse MAC avec ff:fe au milieu).
ptp-port-number = Numéro du port source.
ptp-sequence-id = ID de séquence ; un Follow_Up / Delay_Resp / Pdelay_Resp recopie celui du message auquel il répond.
ptp-control-field = Champ de contrôle (compatibilité IEEE 1588 version 1) : 0 Sync, 1 Delay_Req, 2 Follow_Up, 3 Delay_Resp, 4 Management, 5 autres messages.
ptp-log-message-interval = Log2 de l'intervalle entre messages en secondes (signé ; 0x7f si non applicable).
ptp-timestamp-seconds = Horodatage : secondes (48 bits).
ptp-timestamp-nanoseconds = Horodatage : nanosecondes (32 bits).
ptp-port-identity-clock = Identité de port : identité d'horloge (tous les bits à 1 = toutes les horloges).
ptp-port-identity-port = Identité de port : numéro de port (0xffff = tous les ports).
ptp-current-utc-offset = Décalage UTC actuel en secondes (TAI - UTC).
ptp-grandmaster-priority = Priorité du grandmaster (algorithme BMCA, la plus faible l'emporte).
ptp-grandmaster-clock-class = Classe d'horloge du grandmaster (6 = verrouillée sur une référence primaire, 248 = par défaut, 255 = esclave uniquement).
ptp-grandmaster-clock-accuracy = Précision de l'horloge du grandmaster (0x20 = à 25 ns près ... 0xfe = inconnue).
ptp-grandmaster-variance = offsetScaledLogVariance du grandmaster.
ptp-grandmaster-identity = Identité d'horloge du grandmaster.
ptp-steps-removed = Nombre d'horloges frontières entre le grandmaster et l'émetteur.
ptp-time-source = Source de temps du grandmaster : horloge atomique, GPS, radio terrestre, PTP, NTP, réglage manuel, autre, oscillateur interne.
ptp-starting-boundary-hops = Sauts de frontière initiaux d'un message Management.
ptp-boundary-hops = Sauts de frontière restants.
ptp-action = Action Management : GET, SET, RESPONSE, COMMAND, ACKNOWLEDGE.
ptp-tlvs = TLV (type sur 16 bits, longueur de la valeur sur 16 bits, Auto) : Management, Management Error Status, Organization Extension, négociation unicast (Request / Grant / Cancel / Acknowledge Cancel), Path Trace, Alternate Time Offset Indicator. Ils s'arrêtent à la longueur du message.
ptp-data = Reste du message après l'en-tête commun, en octets (jusqu'à la longueur du message).
esmc-subtype = Sous-type Slow Protocols (0x0a = propre à l'organisation).
esmc-itu-oui = OUI de l'ITU-T (00-19-a7).
esmc-itu-subtype = Sous-type ITU-T (1 = ESMC).
esmc-version = Version ESMC (1).
esmc-event-flag = Indicateur d'événement : 1 pour une PDU d'événement envoyée immédiatement après un changement de QL, 0 pour la PDU d'information envoyée chaque seconde.
esmc-tlvs = TLV (type sur 8 bits, longueur sur 16 bits incluant l'en-tête de 3 octets, Auto) : TLV QL avec le code SSM, TLV QL étendu avec le code SSM amélioré, identité d'horloge SyncE et nombres d'horloges en cascade. Les octets qui suivent (bourrage) sont conservés comme un seul élément brut.
cfm-md-level = Niveau de domaine de maintenance / MEG (0-7).
cfm-version = Version CFM (0 ; 1 pour R-APS de G.8032v2).
cfm-opcode = OpCode : 1 CCM, 2 LBR, 3 LBM, 4 LTR, 5 LTM, 33 AIS, 35 LCK, 37 TST, 39 Linear APS, 40 R-APS, 41 MCC, 42 LMR, 43 LMM, 45 1DM, 46 DMR, 47 DMM, 52 CSF, 53 1SL, 54 SLR, 55 SLM ...
cfm-flags = Indicateurs (propres à l'OpCode).
cfm-first-tlv-offset = Offset du premier TLV depuis la fin de ce champ (constant pour chaque OpCode).
cfm-rdi = RDI : indication de défaut distant.
cfm-period = Période de transmission : 1 = 3.33 ms, 2 = 10 ms, 3 = 100 ms, 4 = 1 s, 5 = 10 s, 6 = 1 min, 7 = 10 min.
cfm-sequence-number = Numéro de séquence.
cfm-mep-id = MEP ID (1-8191).
cfm-maid = MAID / MEG ID, 48 octets : format et nom du MD, format, longueur et nom court du MA, complétés par des zéros (Y.1731 : pas de nom de MD, format 32 ICC-based, longueur 13).
cfm-frame-counter = Compteur de trames (mesure des pertes).
cfm-transaction-id = Identifiant de transaction loopback / linktrace.
cfm-use-fdb-only = UseFDBonly : seule la base de transmission décide du port de sortie.
cfm-fwd-yes = FwdYes : le LTM a été relayé.
cfm-terminal-mep = TerminalMEP : la réponse provient du MEP cible.
cfm-ttl = TTL du linktrace.
cfm-relay-action = Action de relais : 1 RlyHit, 2 RlyFDB, 3 RlyMPDB.
cfm-original-mac = Adresse MAC du MEP ayant lancé le linktrace.
cfm-target-mac = Adresse MAC cible du linktrace.
cfm-csf-type = Type de défaillance du signal client : 0 LOS, 1 FDI / AIS, 2 RDI, 3 DCI.
cfm-timestamp = Horodatage au format IEEE 1588 (secondes et nanosecondes sur 32 bits).
cfm-source-mep-id = MEP ID source.
cfm-responder-mep-id = MEP ID du répondeur (0 dans un SLM).
cfm-test-id = ID de test de la mesure synthétique des pertes.
cfm-tlvs = TLV (type sur 8 bits, longueur de la valeur sur 16 bits, Auto) : Sender ID, Port Status, Data, Interface Status, Reply Ingress / Egress, LTM / LTR Egress Identifier, Organization-Specific, Test, terminés par le TLV End (type 0).
cfm-data = Reste de la PDU après l'offset du premier TLV, en octets (champs propres à l'OpCode et TLV).
raps-request-state = Requête / état : 0000 NR, 0111 MS, 1011 SF, 1101 FS, 1110 Event.
raps-sub-code = Sous-code (0000 = demande de purge avec la requête Event).
raps-rb = RB : le RPL est bloqué.
raps-dnf = DNF : ne pas purger la base de transmission.
raps-bpr = BPR : référence du port bloqué (0 = lien d'anneau 0, 1 = lien d'anneau 1).
raps-node-id = Node ID : adresse MAC du nœud d'anneau émetteur.
efm-oam-subtype = Sous-type Slow Protocols (3 = OAM).
efm-oam-flags = Indicateurs : link fault, dying gasp, critical event, évaluation locale / distante et stable.
efm-oam-code = Code OAMPDU : 0x00 Information, 0x01 Event Notification, 0x02 Variable Request, 0x03 Variable Response, 0x04 Loopback Control, 0xfe Organization Specific.
efm-oam-information-tlvs = TLV d'information (type sur 8 bits, longueur sur 8 bits en-tête compris, Auto) : Local / Remote Information, Organization Specific, terminés par le TLV End (type 0).
efm-oam-sequence-number = Numéro de séquence de notification d'événement.
efm-oam-event-tlvs = TLV d'événement de lien (type sur 8 bits, longueur sur 8 bits en-tête compris, Auto) : errored symbol period, errored frame, errored frame period, errored frame seconds summary, Organization Specific, terminés par le TLV End (type 0).
efm-oam-variable-descriptors = Descripteurs de variables en octets : branche (1 octet) et feuille (2 octets) pour chacun, terminés par une branche nulle.
efm-oam-variable-containers = Conteneurs de variables en octets : branche, feuille, largeur et valeur pour chacun, terminés par une branche nulle.
efm-oam-loopback-command = Commande de bouclage distant : 1 activer, 2 désactiver.
efm-oam-oui = Identifiant unique d'organisation (OUI).
efm-oam-data = Reste de l'OAMPDU, en octets.

# --- P2 : descriptions des champs MACsec / GTPv2-C / Diameter / SIP / RTP ---
macsec-v = Bit V du TCI : version (0).
macsec-es = Bit ES du TCI : End Station (le SCI est l'adresse MAC source avec le port 1).
macsec-sc = Bit SC du TCI : un SCI explicite est présent (Auto : positionné quand le SCI est fourni).
macsec-scb = Bit SCB du TCI : diffusion single copy EPON.
macsec-e = Bit E du TCI : les données sécurisées sont chiffrées.
macsec-c = Bit C du TCI : les données utilisateur ont été modifiées (toujours avec E pour les suites de chiffrement par défaut).
macsec-an = Numéro d'association (0-3) : l'association sécurisée au sein du canal.
macsec-sl = Short length : longueur des données sécurisées si elle est inférieure à 48 octets, sinon 0 (Manuel).
macsec-pn = Numéro de paquet (32 bits, la moitié basse avec XPN).
macsec-sci = Secure Channel Identifier : identifiant système (adresse MAC) et identifiant de port ; présent quand SC est positionné.
macsec-secure-data = Données sécurisées : les données utilisateur chiffrées (ou seulement protégées en intégrité), en octets opaques ; rien n'est chiffré.
macsec-icv = Integrity Check Value (16 octets), opaque : elle n'est pas calculée.
gtpv2-version = Version GTP (2).
gtpv2-p = Indicateur P : un message piggyback suit (la couche GTPv2-C suivante).
gtpv2-t = Indicateur T : le champ TEID est présent (Auto : positionné quand un TEID est fourni).
gtpv2-mp = Indicateur MP : la priorité du message est présente.
gtpv2-message-type = Type de message (Echo, Create / Modify / Delete Session, Create / Update / Delete Bearer, ...).
gtpv2-length = Longueur du message en octets, sans les 4 premiers octets (Auto).
gtpv2-teid = Tunnel Endpoint Identifier du destinataire (absent des messages Echo ; 0 dans une requête initiale).
gtpv2-sequence = Numéro de séquence (24 bits) associant une réponse à sa requête.
gtpv2-message-priority = Priorité du message (0 la plus haute), utilisée quand MP est positionné.
gtpv2-ies = Éléments d'information (type sur 8 bits, longueur des données sur 16 bits, octet spare / instance) : IMSI, Cause, Recovery, APN, AMBR, EBI, IP Address, MEI, MSISDN, PAA, Bearer QoS, RAT Type, Serving Network, F-TEID, Bearer Context (groupé), PDN Type, ... Les longueurs sont Auto.
diameter-version = Version Diameter (1).
diameter-length = Longueur du message en octets, en-tête et bourrage des AVP compris (Auto).
diameter-r = Indicateur R : requête (0 = réponse).
diameter-p = Indicateur P : proxiable.
diameter-e = Indicateur E : la réponse contient une erreur de protocole.
diameter-t = Indicateur T : message potentiellement retransmis.
diameter-command-code = Code de commande (Capabilities-Exchange, Device-Watchdog, Credit-Control, ...).
diameter-application-id = Application-ID (0 = messages communs Diameter).
diameter-hop-by-hop = Identifiant Hop-by-Hop associant une réponse à sa requête sur une connexion.
diameter-end-to-end = Identifiant End-to-End permettant de détecter les requêtes en double.
diameter-avps = AVP (code sur 32 bits, indicateurs V / M / P, longueur sur 24 bits hors bourrage, Vendor-ID optionnel, données complétées à 4 octets) : AVP du protocole de base et de Credit-Control, AVP groupés sur un niveau. Les longueurs, le bourrage et l'indicateur V sont Auto.
sip-start-line = Ligne de début : une ligne de requête (méthode, Request-URI, SIP/2.0) ou une ligne de statut (SIP/2.0, code, motif). Le CRLF est ajouté automatiquement.
sip-headers = Lignes d'en-tête (Nom: valeur), une par élément ; les CRLF et la ligne vide qui les suit sont ajoutés automatiquement. Content-Length est Manuel.
sip-body = Corps du message (une offre / réponse SDP, ...).
rtp-version = Version RTP (2).
rtp-padding = Bit P : octets de bourrage à la fin de la charge utile (le dernier indique leur nombre, Manuel).
rtp-x = Bit X : une extension d'en-tête suit la liste CSRC (Auto).
rtp-cc = Nombre de CSRC (Auto).
rtp-marker = Bit marqueur (défini par le profil, p. ex. le début d'un talkspurt).
rtp-payload-type = Type de charge utile (0 PCMU, 8 PCMA, 9 G.722, 18 G.729, 96-127 dynamique).
rtp-sequence = Numéro de séquence, incrémenté de un à chaque paquet RTP.
rtp-timestamp = Horodatage : instant d'échantillonnage du premier octet de la charge utile.
rtp-ssrc = Identifiant de source de synchronisation.
rtp-csrc = Identifiants des sources contributrices (15 au plus).
rtp-extension = Extension d'en-tête : 16 bits définis par le profil (0xBEDE : éléments sur un octet de la RFC 8285), la longueur en mots de 32 bits (Auto) et les mots d'extension.
rtcp-version = Version RTCP (2).
rtcp-padding = Bit P : octets de bourrage à la fin du paquet (Manuel).
rtcp-count = Champ count / format (5 bits) d'un type de paquet non typé.
rtcp-report-count = Nombre de rapports de réception (Auto).
rtcp-source-count = Nombre de sources : chunks SDES ou identifiants BYE (Auto).
rtcp-subtype = Sous-type défini par l'application.
rtcp-packet-type = Type de paquet (200 SR, 201 RR, 202 SDES, 203 BYE, 204 APP, 205 RTPFB, 206 PSFB, 207 XR).
rtcp-length = Longueur du paquet en mots de 32 bits moins un (Auto).
rtcp-body = Corps du paquet après l'en-tête commun, en octets.
rtcp-ssrc = SSRC de l'émetteur de ce paquet.
rtcp-ntp-timestamp = Horodatage NTP (mot de poids fort ; le champ suivant est la fraction).
rtcp-rtp-timestamp = Horodatage RTP correspondant à l'horodatage NTP.
rtcp-packet-count = Nombre de paquets de l'émetteur.
rtcp-octet-count = Nombre d'octets de charge utile de l'émetteur.
rtcp-report-blocks = Blocs de rapport de réception : SSRC, fraction perdue, pertes cumulées, plus grand numéro de séquence étendu, gigue, LSR, DLSR.
rtcp-extension = Octets d'extension propres au profil, jusqu'à la longueur du paquet.
rtcp-chunks = Chunks SDES : SSRC / CSRC et ses éléments (type, longueur (Auto), texte), terminés par un octet nul et complétés à 32 bits.
rtcp-ssrcs = Identifiants SSRC / CSRC quittant la session.
rtcp-reason = Motif de départ facultatif : longueur (Auto), texte, bourrage à 32 bits.
rtcp-app-name = Nom (4 caractères ASCII).
rtcp-app-data = Données dépendantes de l'application (multiple de 4 octets).

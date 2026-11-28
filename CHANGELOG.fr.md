# Historique des versions

[English](CHANGELOG.md) · **Français**

Les changements notables du projet sont consignés ici.
Le format suit [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/) et le projet utilise le [versionnage sémantique](https://semver.org/lang/fr/).

## [0.3.0] - 2026-11-22

### Ajouté
- **Trois nouveaux sites : Thangs, Cults et MyMiniFactory** (six sites au total), avec leurs icônes. **Thangs** se recherche sans compte en lisant sa page de résultats avec le navigateur d'arrière-plan (titre, image et lien ; lien *Ouvrir* seulement). **Cults** et **MyMiniFactory** bloquent la navigation automatisée (contrôle anti-robot Cloudflare, détection de robots) et demandent aux applications d'utiliser leurs API officielles : ils sont donc recherchés par ce moyen, **avec votre propre clé d'API gratuite** (Paramètres → Cults / MyMiniFactory, ou `login cults3d --token <nom_utilisateur>:<cle_api>`). Tant qu'aucune clé n'est enregistrée, ils sont ignorés, avec un message d'information plutôt qu'une erreur. Les résultats Cults incluent auteur, likes, téléchargements et prix. L'intégration MyMiniFactory n'a pas pu être testée avec une vraie clé.
- **Interface en allemand** (Deutsch), en plus de l'anglais et du français, aussi utilisée pour le menu de l'icône.
- **Réglages réseau pour un serveur sans écran** : `PRINTSEARCH_BIND` (adresse d'écoute, `127.0.0.1` par défaut) et `PRINTSEARCH_ALLOWED_HOSTS` (noms d'hôte acceptés dans les en-têtes `Host` / `Origin`). Quand le serveur écoute au-delà de cet ordinateur, les requêtes des **autres ordinateurs sont limitées à la recherche** ; comptes, console, journaux, choix du navigateur et *Quitter* restent réservés à l'ordinateur local.
- Section du README sur l'utilisation d'un serveur Linux sans écran, avec un exemple systemd.
- **Fonctionne en arrière-plan, sans fenêtre de terminal.** Sous **Windows**, le programme n'ouvre plus de console (c'est un programme fenêtré ; lancé depuis un terminal, il s'y rattache pour que les commandes affichent leur résultat). Une **icône de barre d'état** (barre des tâches Windows, zone de notification Linux via StatusNotifierItem) ouvre l'interface au clic gauche et propose *Ouvrir PrintSearch* et *Quitter* au clic droit. La langue du menu suit celle de l'interface (appliquée au prochain démarrage).
- **Lancé depuis un terminal, PrintSearch se détache tout seul** (Linux) : il se relance dans sa propre session, sans terminal, vous rend l'invite et continue de tourner quand vous fermez le terminal. `--foreground` le garde attaché au terminal.
- **Instance unique :** lancer PrintSearch alors qu'il tourne déjà ouvre simplement l'instance existante.
- **Console intégrée**, accessible depuis **Paramètres → Paramètres avancés → Ouvrir la console** (nouvelle page *Paramètres avancés*, qui contient aussi le téléchargement des journaux), qui accepte **les mêmes commandes que le terminal**, avec historique (↑/↓), complétion par Tab, `clear` et `lang en|fr`. Un jeton tapé dans une commande est masqué à l'écran et dans le journal, et ces commandes ne sont pas gardées dans l'historique.
- **Journal de l'application** et les commandes associées : `logs` (affichage, `-n`, `--level`), `logs export [chemin]` (journal complet avec un en-tête de diagnostic, vers votre dossier Téléchargements par défaut), `logs path`, `logs clear`. Aussi **Paramètres → Paramètres avancés → Télécharger les journaux**. Le journal consigne les recherches, erreurs, démarrage du navigateur, mises à jour et plantages ; il ne contient jamais de jeton. Il est limité à environ 2 Mo.
- Nouvelles commandes : `status` (version, navigateur, connexions, emplacement du journal) et `quit` (ferme le PrintSearch en cours).
- Carte **Paramètres → Application** avec un bouton *Quitter PrintSearch*.
- `serve --no-tray` pour tourner dans le terminal sans icône (Ctrl+C ou SIGTERM l'arrêtent proprement).
- Fonction Cargo `tray` (activée par défaut). `--no-default-features` compile un programme de terminal seul.

### Modifié
- Les messages et commentaires du code sont maintenant en anglais, pour que les contributeurs puissent les lire.
- **Les résultats s'affichent site par site** : chaque site est interrogé séparément, donc un site rapide s'affiche tout de suite au lieu d'attendre le plus lent. Les sites qui échouent pour la même raison (par exemple aucun navigateur installé) partagent un seul message d'erreur.
- Les cartes n'affichent les boutons **Like** et **Collection** que sur les sites où ils existent (Thingiverse et Printables pour Like, Thingiverse pour les collections) ; la ligne de commande et l'API donnent un message clair dans les autres cas.
- La protection Host/Origin accepte désormais les noms d'hôte listés dans `PRINTSEARCH_ALLOWED_HOSTS`, et l'API refuse les identifiants de modèle et de collection qui ne sont pas faits de lettres, chiffres, `-` et `_`.
- Le navigateur d'arrière-plan lit **au plus trois pages en même temps**, pour que la recherche de cinq sites lus par navigateur ne le surcharge pas.
- `printsearch` (sans argument) et `serve` lancent maintenant l'application en arrière-plan. Le lanceur `.desktop` utilise `Terminal=false`.
- Le navigateur d'arrière-plan n'est plus lancé au démarrage : il démarre à la première recherche qui en a besoin et est **fermé après 10 minutes d'inactivité**, pour qu'une application résidente ne garde pas un navigateur ouvert pendant des jours.
- L'arrêt est propre : le navigateur d'arrière-plan est fermé et le serveur s'arrête en moins de 6 secondes.
- La console n'est plus un bouton de l'écran principal ni une entrée du menu de l'icône : elle a été déplacée dans **Paramètres → Paramètres avancés**.
- L'aide et les messages de la ligne de commande sont maintenant en anglais (l'interface web garde son choix anglais / français).
- READMEs : icône, console, commandes, journaux, dépannage.

### Corrigé
- Un **processus de navigateur pouvait rester actif** après une recherche en ligne de commande ou à la fermeture de l'application.
- Un **`config.json` corrompu** empêchait l'application de démarrer ; il est maintenant mis de côté sous `config.json.bad` et l'application démarre avec les valeurs par défaut.
- La page de retour OAuth de Thingiverse acceptait n'importe quel code d'autorisation : elle n'en accepte un que si la connexion a été lancée depuis l'application (dans les 10 minutes).
- Les identifiants de modèle et de collection sont validés avant d'être placés dans les adresses d'API, et les requêtes vers l'API locale sont limitées à 64 Ko.
- Un terminal bloqué pouvait figer la journalisation dans les autres threads.
- Fermer le terminal d'un PrintSearch lancé avec `--foreground` ou `--no-tray` l'arrête maintenant proprement (SIGHUP est géré) au lieu de le faire planter.

### Remarques
- **macOS** n'a pas d'icône de barre d'état dans cette version (il tourne dans le terminal).
- Sous **GNOME**, l'icône nécessite l'extension *AppIndicator and KStatusNotifierItem Support*. Sans zone de notification, l'application tourne quand même ; utilisez l'interface web ou `printsearch quit`.
- Sous Windows, l'icône peut être rangée dans la zone de débordement (`^`) ; glissez-la sur la barre des tâches.

## [0.2.0] - 2026-11-20

### Ajouté
- **Choix du navigateur utilisé pour les recherches.** MakerWorld (et Thingiverse sans jeton) sont lus avec un navigateur basé sur Chromium qui tourne en arrière-plan. Vous pouvez maintenant le choisir dans **⚙ Paramètres → Navigateur utilisé pour les recherches** : *Automatique*, un navigateur détecté (**Chrome, Edge, Brave, Vivaldi, Chromium**) ou un **chemin personnalisé**. Chrome n'est plus obligatoire : **Edge, préinstallé sous Windows, suffit**.
- Commande `printsearch browser` pour lister les navigateurs détectés et en sélectionner un (`auto`, `chrome`, `edge`, `brave`, `vivaldi`, `chromium` ou un chemin).
- **Icône de l'application intégrée à l'`.exe` Windows** (`build.rs`, `packaging/printsearch.ico`). Si l'icône ne peut pas être intégrée (par exemple `windres` absent en compilation croisée), la compilation réussit quand même et affiche un avertissement.
- **Résultats progressifs :** chaque site est interrogé séparément, donc les sites rapides (par exemple Printables) s'affichent tout de suite au lieu d'attendre le plus lent. Un message de progression indique combien de sites ont répondu, et l'erreur d'un site ne masque plus les autres.
- **Vérification des mises à jour** auprès du dépôt GitHub : *Paramètres → Mises à jour → Rechercher des mises à jour*, commande `printsearch update`, et vérification facultative au démarrage (activée par défaut, désactivable) qui affiche une notification à fermer quand une version plus récente existe. Elle lit la dernière release publiée (avec repli sur la page publique des releases si la limite de requêtes de l'API est atteinte) et ne télécharge ni n'installe jamais rien.
- `printsearch --version`.
- Ce changelog, en anglais et en français.

### Modifié
- Les résultats sont fusionnés côté interface (sites entrelacés, puis triés par likes ou téléchargements si demandé). Les cartes déjà affichées sont réutilisées : plus de clignotement, et leur animation d'apparition n'est plus rejouée. Un même modèle présent deux fois n'est affiché qu'une fois.
- Un seul client HTTP est partagé par tout le programme (connexions réutilisées) et il a désormais des délais d'attente : 8 s pour se connecter, 25 s par requête.
- Le cache du lecteur de pages est plafonné (40 entrées) et les entrées expirées sont purgées.
- Les connexions HTTPS font maintenant aussi confiance au magasin de certificats du système d'exploitation (et plus seulement à celui embarqué), donc l'application fonctionne derrière un proxy d'entreprise ou un antivirus qui inspecte le HTTPS.
- Le fichier de configuration est écrit de façon atomique (fichier temporaire puis renommage).
- Le paquet `.deb` recommande maintenant aussi Edge et Brave, en plus de Chromium et Chrome.
- Le mainteneur et l'auteur du paquet sont désormais **Print-lab**.
- READMEs mis à jour (choix du navigateur, icône Windows, compilation croisée depuis Linux, `rustfmt` requis pour compiler).

### Sécurité
- Le serveur local rejette toute requête dont le `Host` n'est pas `localhost:8787` / `127.0.0.1:8787`, et toute requête avec une `Origin` étrangère. Cela protège contre les pages web malveillantes ouvertes dans votre navigateur (CSRF et « DNS rebinding »), qui auraient pu appeler l'API locale, par exemple pour changer l'exécutable du navigateur.

### Corrigé
- **Blocage quand le navigateur sélectionné n'est pas un vrai navigateur** (mauvais chemin, programme non basé sur Chromium) : le lancement pouvait bloquer indéfiniment et figer les recherches suivantes. Le lancement échoue maintenant après 35 s avec un message clair, un échec de lancement est mémorisé 20 s au lieu d'être retenté en boucle, chaque appel au navigateur a un délai de 10 s, et la lecture d'une page est plafonnée à 75 s.
- Une recherche lancée sans aucun site sélectionné pouvait afficher les résultats de la recherche précédente.
- Un plantage dans un thread ne peut plus désactiver définitivement le lecteur de pages (les verrous empoisonnés sont récupérés).
- Le démarrage d'une seconde instance indique clairement que le port 8787 est déjà utilisé.

### Remarques
- Firefox et Safari ne peuvent pas servir aux recherches (le lecteur exige un navigateur basé sur Chromium).
- La compilation nécessite `rustfmt` (inclus dans une installation `rustup` normale) à cause de la dépendance `headless_chrome`.
- Limites inchangées : les points d'accès de Printables et MakerWorld sont non officiels, et Like / collections restent expérimentaux (voir le README).

## [0.1.0]

Première version.

### Ajouté
- Recherche sur **Thingiverse, Printables et MakerWorld** depuis une interface web locale et en ligne de commande.
- Filtres (sites, prix, tri, likes et téléchargements minimum, résultats par site) avec relance automatique de la recherche, pagination (« Charger plus ») et animations.
- Actions Like et ajout à une collection (expérimentales), connexion aux sites depuis une page Paramètres (App Token ou OAuth pour Thingiverse, jeton Bearer pour Printables).
- Interface en anglais et en français (anglais par défaut) et historique de recherche désactivable.
- Packaging : métadonnées `.deb`, lanceur et icône ; compilation Windows. Publié sous licence GPL v3.

[0.3.0]: https://github.com/Print-lab/PrintSearch/releases/tag/v0.3.0
[0.2.0]: https://github.com/Print-lab/PrintSearch/releases/tag/v0.2.0
[0.1.0]: https://github.com/Print-lab/PrintSearch/releases/tag/v0.1.0

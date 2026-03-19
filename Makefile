# NOTE: les arguments précédés par des moins (-y, --version) seront capturés par make et ne seront pas disponibles pour les commandes
#
# On peut les forcer en ajoutant un argument "--" :
# Tout ce qui suit cet argument spécial n'est pas capturé par make, et sera donc correctement envoyé vers les commandes
#
# La notation générique permet de complètement contourner ce problème : make [action] -- [arguments]
# exemples :
#   make drush -- cim -y
#   make npm -- install malib --save-dev


## GESTION DES CONTAINERS
.PHONY: build release docker

build:
	@docker build -t deepseek-agent .
agent:
	@docker run --rm -it \
		-v ./workspace:/app \
		-w /app \
		-u $(shell id -u):$(shell id -g) \
		--env-file .env \
		deepseek-agent

%:
	@:

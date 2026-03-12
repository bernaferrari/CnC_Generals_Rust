//// EstablishConnectionsMenu.h /////////////////////////

#include "GameNetwork/NetworkDefs.h"
#include "GameNetwork/NAT.h"

enum EstablishConnectionsMenuStateType {
	ESTABLISHCONNECTIONSMENUSTATETYPE_SCREENON,
	ESTABLISHCONNECTIONSMENUSTATETYPE_SCREENOFF
};

class EstablishConnectionsMenu {
public:
	EstablishConnectionsMenu();
	virtual ~EstablishConnectionsMenu();

	void initMenu();
	void endMenu();
	void abortGame();

	void setPlayerName(Int slot, UnicodeString name);
	void setPlayerStatus(Int slot, NATConnectionState state);

protected:
	EstablishConnectionsMenuStateType m_menuState;

	static char *m_playerReadyControlNames[MAX_SLOTS];
	static char *m_playerNameControlNames[MAX_SLOTS];
	static char *m_playerStatusControlNames[MAX_SLOTS];
};

extern EstablishConnectionsMenu *TheEstablishConnectionsMenu;
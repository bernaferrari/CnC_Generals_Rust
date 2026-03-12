// FILE: .cpp /////////////////////////////////////////////////////////////////////////////////////
// Author: Colin Day
// Description: Game Client message dispatcher
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "Common/MessageStream.h"
#include "GameClient/GameClient.h"

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
/** The Client message dispatcher, this is the last "translator" on the message
	* stream before the messages go to the network for processing.  It gives
	* the client itself the opportunity to respond to any messages on the stream
	* or create new ones to pass along to the network and logic */
GameMessageDisposition GameClientMessageDispatcher::translateGameMessage(const GameMessage *msg)
{
	if (msg->getType() >= GameMessage::MSG_BEGIN_NETWORK_MESSAGES && msg->getType() <= GameMessage::MSG_END_NETWORK_MESSAGES)
		return KEEP_MESSAGE;
	if (msg->getType() == GameMessage::MSG_NEW_GAME || msg->getType() == GameMessage::MSG_CLEAR_GAME_DATA)
		return KEEP_MESSAGE;

	if (msg->getType() == GameMessage::MSG_FRAME_TICK)
		return KEEP_MESSAGE;

	//DEBUG_LOG(("GameClientMessageDispatcher::translateGameMessage() - eating a %s on frame %d\n",
		//((GameMessage *)msg)->getCommandAsAsciiString().str(), TheGameClient->getFrame()));

	return DESTROY_MESSAGE;
}  // end clientMessageDispatcher

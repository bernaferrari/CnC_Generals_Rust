// FILE: GSConfig.h ///////////////////////////////////////////////////////////
// Author: Matthew D. Campbell, Sept 2002
// Description: GameSpy online config
///////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __GSCONFIG_H__
#define __GSCONFIG_H__

#include "Common/AsciiString.h"
#include "Common/STLTypedefs.h"

class GameSpyConfigInterface
{
public:
	virtual ~GameSpyConfigInterface() {}

	// Pings
	virtual std::list<AsciiString> getPingServers(void) = 0;
	virtual Int getNumPingRepetitions(void) = 0;
	virtual Int getPingTimeoutInMs(void) = 0;
	virtual Int getPingCutoffGood( void ) = 0;
	virtual Int getPingCutoffBad( void ) = 0; //Bryan sez, Maybe

	// QM
	virtual std::list<AsciiString> getQMMaps(void) = 0;
	virtual Int getQMBotID(void) = 0;
	virtual Int getQMChannel(void) = 0;
	virtual void setQMChannel(Int channel) = 0;

	// Player Info
	virtual Int getPointsForRank(Int rank) = 0;
	virtual Bool isPlayerVIP(Int id) = 0;

	// mangler Info
	virtual Bool getManglerLocation(Int index, AsciiString& host, UnsignedShort& port) = 0;

	// Ladder / Any other external parsing
	virtual AsciiString getLeftoverConfig(void) = 0;

	// Custom match
	virtual Bool restrictGamesToLobby() = 0;
	static GameSpyConfigInterface* create(AsciiString config);
};

extern GameSpyConfigInterface *TheGameSpyConfig;

#endif // __GSCONFIG_H__

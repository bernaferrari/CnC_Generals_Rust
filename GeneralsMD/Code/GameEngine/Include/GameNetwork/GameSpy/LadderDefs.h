// FILE: LadderDefs.h //////////////////////////////////////////////////////
// Generals ladder definitions
// Author: Matthew D. Campbell, August 2002

#pragma once

#ifndef __LADDERDEFS_H__
#define __LADDERDEFS_H__

#include "Common/UnicodeString.h"
#include "Common/AsciiString.h"
#include "Common/STLTypedefs.h"

class GameWindow;

class LadderInfo
{
public:
	LadderInfo();
	UnicodeString name;
	UnicodeString description;
	UnicodeString location;
	Int playersPerTeam;
	Int minWins;
	Int maxWins;
	Bool randomMaps;
	Bool randomFactions;
	Bool validQM;
	Bool validCustom;
	std::list<AsciiString> validMaps;
	std::list<AsciiString> validFactions;
	AsciiString cryptedPassword;
	AsciiString address;
	UnsignedShort port;
	AsciiString homepageURL;
	Bool submitReplay; // with game results
	Int index;
};

typedef std::list<LadderInfo *> LadderInfoList;

class LadderList
{
public:
	LadderList();
	~LadderList();

	const LadderInfo* findLadder( const AsciiString& addr, UnsignedShort port );
	const LadderInfo* findLadderByIndex( Int index );	// doesn't look in local ladders
	const LadderInfoList* getLocalLadders( void );
	const LadderInfoList* getSpecialLadders( void );
	const LadderInfoList* getStandardLadders( void );

private:
	void loadLocalLadders( void );
	void checkLadder( AsciiString fname, Int index );
	LadderInfoList m_localLadders;
	LadderInfoList m_specialLadders;
	LadderInfoList m_standardLadders;
};

extern LadderList *TheLadderList;

#endif // __LADDERDEFS_H__

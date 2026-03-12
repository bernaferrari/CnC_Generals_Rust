// FILE: ProductionPrerequisite.h //////////////////////////////////////////////////////
//
// Project:    RTS3
//
// File name:  ProductionPrerequisite.h
//
// Created:    Steven Johnson, October 2001
//
//-----------------------------------------------------------------------------

#pragma once

#ifndef __ProductionPrerequisite_H_
#define __ProductionPrerequisite_H_

//-----------------------------------------------------------------------------
//           Includes                                                      
//-----------------------------------------------------------------------------
#include "Common/GameMemory.h"
#include "Common/GameCommon.h"
#include "Common/Science.h"
//#include "GameClient/ControlBar.h"

//-----------------------------------------------------------------------------
class ThingTemplate;
class Player;

//-----------------------------------------------------------------------------
class ProductionPrerequisite
{
public:

	ProductionPrerequisite();
	~ProductionPrerequisite();

	/// init to safe default values.
	void init();

	void resetSciences( void ) { m_prereqSciences.clear(); }
	void addSciencePrereq( ScienceType science ) { m_prereqSciences.push_back(science); }

	void resetUnits( void ) { m_prereqUnits.clear(); }
	void addUnitPrereq( AsciiString units, Bool orUnitWithPrevious );
	void addUnitPrereq( const std::vector<AsciiString>& units );

	/// called after all ThingTemplates have been loaded.
	void resolveNames();

	/// returns an asciistring which is a list of all the prerequisites
	/// not satisfied yet
	UnicodeString getRequiresList(const Player *player) const;

	/// return true iff the player satisfies our set of prerequisites
	Bool isSatisfied(const Player *player) const;

	/** 
		return the BuildFacilityTemplate, if any. 
		
		if this template needs no build facility, null is returned.

		if the template needs a build facility but the given player doesn't have any in existence,
		null will be returned.

		you may not pass 'null' for player.
	*/
	const ThingTemplate *getExistingBuildFacilityTemplate( const Player *player ) const;

	Int getAllPossibleBuildFacilityTemplates(const ThingTemplate* tmpls[], Int maxtmpls) const;

private:
	
	enum 
	{
		UNIT_OR_WITH_PREV = 0x01	// if set, unit is "or-ed" with prev unit, so that either one's presence satisfies
	};

	struct PrereqUnitRec 
	{
		const ThingTemplate*	unit;
		Int										flags;
		AsciiString						name;
	};

	enum { MAX_PREREQ = 32 };
	Int calcNumPrereqUnitsOwned(const Player *player, Int counts[MAX_PREREQ]) const;

	std::vector<PrereqUnitRec>	m_prereqUnits;
	ScienceVec									m_prereqSciences;
};

//-----------------------------------------------------------------------------

#endif

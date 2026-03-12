// FILE: Handicap.cpp /////////////////////////////////////////////////////////
//
// Project:   RTS3
//
// File name: Handicap.cpp
//
// Created:   Steven Johnson, October 2001
//
// Desc:      @todo
//
//-----------------------------------------------------------------------------

#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "Common/Handicap.h"
#include "Common/Player.h"
#include "Common/Dict.h"
#include "Common/ThingTemplate.h"

//-----------------------------------------------------------------------------
Handicap::Handicap() 
{
	init();
}

//-----------------------------------------------------------------------------
void Handicap::init()
{
	for (Int i = 0; i < HANDICAP_TYPE_COUNT; ++i)
		for (Int j = 0; j < THING_TYPE_COUNT; ++j)
			m_handicaps[i][j] = 1.0f;
}

//-----------------------------------------------------------------------------
void Handicap::readFromDict(const Dict* d)
{
	// this isn't very efficient, but is only called at load times, 
	// so it probably doesn't really matter.

	const char* htNames[HANDICAP_TYPE_COUNT] = 
	{
		"BUILDCOST",
		"BUILDTIME",
//		"FIREPOWER",
//		"ARMOR",
//		"GROUNDSPEED",
//		"AIRSPEED",
//		"INCOME"
	};

	const char* ttNames[THING_TYPE_COUNT] = 
	{
		"GENERIC",
		"BUILDINGS",
	};

// no, you should NOT call init() here.
//init();

	AsciiString c;
	for (Int i = 0; i < HANDICAP_TYPE_COUNT; ++i)
	{
		for (Int j = 0; j < THING_TYPE_COUNT; ++j)
		{
			c.clear();
			c.set("HANDICAP_");
			c.concat(htNames[i]);
			c.concat("_");
			c.concat(ttNames[j]);
			NameKeyType k = TheNameKeyGenerator->nameToKey(c);
			Bool exists;
			Real r = d->getReal(k, &exists);
			if (exists)
				m_handicaps[i][j] = r;
		}
	}
}

//-----------------------------------------------------------------------------
/*static*/ Handicap::ThingType Handicap::getBestThingType(const ThingTemplate *tmpl)
{
	/// if this ends up being too slow, cache the information in the object
	if (tmpl->isKindOf(KINDOF_STRUCTURE))
		return BUILDINGS;

	return GENERIC;
}

//-----------------------------------------------------------------------------
Real Handicap::getHandicap(HandicapType ht, const ThingTemplate *tmpl) const
{
	ThingType tt = getBestThingType(tmpl);
	return m_handicaps[ht][tt];
}

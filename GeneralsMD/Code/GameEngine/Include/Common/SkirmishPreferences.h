///////////////////////////////////////////////////////////////////////////////////////
// FILE: SkirmishPreferences.h
// Author: Chris Huybregts, August 2002
// Description: Saving/Loading of skirmish preferences
///////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __SKIRMISHPREFERENCES_H__
#define __SKIRMISHPREFERENCES_H__

//-----------------------------------------------------------------------------
// USER INCLUDES //////////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------
#include "Common/UserPreferences.h"

//-----------------------------------------------------------------------------
// SkirmishPreferences class 
//-----------------------------------------------------------------------------
class SkirmishPreferences : public UserPreferences
{
public:
	SkirmishPreferences();
	virtual ~SkirmishPreferences();
	virtual Bool write(void);
	AsciiString getSlotList(void);
	void setSlotList(void);
	UnicodeString getUserName(void);		// convenience function
	Int getPreferredFaction(void);			// convenience function
	Int getPreferredColor(void);				// convenience function
	AsciiString getPreferredMap(void);	// convenience function
	Bool usesSystemMapDir(void);		// convenience function
  
  Bool getSuperweaponRestricted(void) const;
  void setSuperweaponRestricted( Bool superweaponRestricted);
  
  Money getStartingCash(void) const;
  void setStartingCash( const Money &startingCash );
};

#endif // __SKIRMISHPREFERENCES_H__

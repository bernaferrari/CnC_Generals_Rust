///////////////////////////////////////////////////////////////////////////////////////
// FILE: CustomMatchPreferences.h
// Author: Matthew D. Campbell, August 2002
// Description: Saving/Loading of custom match preferences
///////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __CUSTOMMATCHPREFERENCES_H__
#define __CUSTOMMATCHPREFERENCES_H__

//-----------------------------------------------------------------------------
// USER INCLUDES //////////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------
#include "Common/UserPreferences.h"

//-----------------------------------------------------------------------------
// CustomMatchPreferences base class 
//-----------------------------------------------------------------------------
class CustomMatchPreferences : public UserPreferences
{
public:
	CustomMatchPreferences();
	virtual ~CustomMatchPreferences();

	void setLastLadder(const AsciiString& addr, UnsignedShort port);
	AsciiString getLastLadderAddr( void );
	UnsignedShort getLastLadderPort( void );

	Int getPreferredFaction(void);
	void setPreferredFaction(Int val);

	Int getPreferredColor(void);
	void setPreferredColor(Int val);

	Int getChatSizeSlider(void);
	void setChatSizeSlider(Int val);

	AsciiString getPreferredMap(void);
	void setPreferredMap(AsciiString val);

	Bool usesSystemMapDir(void);
	void setUsesSystemMapDir(Bool val);

	Bool usesLongGameList(void);
	void setUsesLongGameList(Bool val);

	Bool allowsObservers(void);
	void setAllowsObserver(Bool val);

	Bool getDisallowAsianText( void );
	void setDisallowAsianText(Bool val);

	Bool getDisallowNonAsianText( void );
	void setDisallowNonAsianText( Bool val );

  Bool getSuperweaponRestricted(void) const;
  void setSuperweaponRestricted( Bool superweaponRestricted);

  Money getStartingCash(void) const;
  void setStartingCash( const Money &startingCash );

  Bool getFactionsLimited(void) const; // Prefers to only use the original 3 sides, not USA Air Force General, GLA Toxin General, et al
  void setFactionsLimited( Bool factionsLimited );
  
  Bool getUseStats( void ) const;
  void setUseStats( Bool useStats );
};

#endif // __CUSTOMMATCHPREFERENCES_H__

///////////////////////////////////////////////////////////////////////////////////////
// FILE: GameSpyMiscPreferences.h
// Author: Matthew D. Campbell, December 2002
// Description: Saving/Loading of misc preferences
///////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __GAMESPYMISCPREFERENCES_H__
#define __GAMESPYMISCPREFERENCES_H__

//-----------------------------------------------------------------------------
// USER INCLUDES //////////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------
#include "Common/UserPreferences.h"

//-----------------------------------------------------------------------------
// GameSpyMiscPreferences base class 
//-----------------------------------------------------------------------------
class GameSpyMiscPreferences : public UserPreferences
{
public:
	GameSpyMiscPreferences();
	virtual ~GameSpyMiscPreferences();

	Int getLocale( void );
	void setLocale( Int val );

	AsciiString getCachedStats( void );
	void setCachedStats( AsciiString val );

	Bool getQuickMatchResLocked( void );

	Int getMaxMessagesPerUpdate( void );
};

#endif // __GAMESPYMISCPREFERENCES_H__

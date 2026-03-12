// FILE: W3DDisplayStringManager.h ////////////////////////////////////////////////////////////////
// Created:    Colin Day, July 2001
// Desc:       Access for creating game managed display strings
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef _W3DDISPLAYSTRINGMANAGER_H_
#define _W3DDISPLAYSTRINGMANAGER_H_

#include "GameClient/DisplayStringManager.h"
#include "W3DDevice/GameClient/W3DDisplayString.h"

//-------------------------------------------------------------------------------------------------
/** Access for creating game managed display strings */
//-------------------------------------------------------------------------------------------------
//#define KRIS_BRUTAL_HACK_FOR_AIRCRAFT_CARRIER_DEBUGGING

#ifdef KRIS_BRUTAL_HACK_FOR_AIRCRAFT_CARRIER_DEBUGGING
	#define MAX_GROUPS 20
#else
	#define MAX_GROUPS 10
#endif

class W3DDisplayStringManager : public DisplayStringManager
{

public:

	W3DDisplayStringManager( void );
	virtual ~W3DDisplayStringManager( void );

	// Initialize our numeral strings in postProcessLoad
	virtual void postProcessLoad( void );

	/// update method for all our display strings
	virtual void update( void );

	/// allocate a new display string
	virtual DisplayString *newDisplayString( void );

	/// free a display string
	virtual void freeDisplayString( DisplayString *string );
	
	// This is used to save us a few FPS and storage space. There's no reason to 
	// duplicate the DisplayString on every drawable when 1 copy will suffice.
	virtual DisplayString *getGroupNumeralString( Int numeral );
	virtual DisplayString *getFormationLetterString( void ) { return m_formationLetterDisplayString; };

protected:
	DisplayString *m_groupNumeralStrings[ MAX_GROUPS ];
	DisplayString *m_formationLetterDisplayString;

};

#endif // _W3DDISPLAYSTRINGMANAGER_H_


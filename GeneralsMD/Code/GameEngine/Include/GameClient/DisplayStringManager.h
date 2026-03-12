// FILE: DisplayStringManager.h ///////////////////////////////////////////////////////////////////
// Created:    Colin Day, July 2001
// Desc:       Access for creating game managed display strings
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __DISPLAYSTRINGMANAGER_H_
#define __DISPLAYSTRINGMANAGER_H_

#include "Common/SubsystemInterface.h"
#include "GameClient/DisplayString.h"

//-------------------------------------------------------------------------------------------------
/** Factory for managing and creating display strings */
//-------------------------------------------------------------------------------------------------
class DisplayStringManager : public SubsystemInterface
{

public:

	DisplayStringManager( void );
	virtual ~DisplayStringManager( void );

	virtual void init( void ) {}			///< initialize the factory
	virtual void reset( void ) {}			///< reset system
	virtual void update( void ) {};		///< update anything we need to in our strings

	virtual DisplayString *newDisplayString( void ) = 0;  ///< allocate new display string
	virtual void freeDisplayString( DisplayString *string ) = 0;  ///< free string

	virtual DisplayString *getGroupNumeralString( Int numeral ) = 0;
	virtual DisplayString *getFormationLetterString( void ) = 0;
protected:

	void link( DisplayString *string );  ///< link display string to list
	void unLink( DisplayString *string );  ///< unlink display string from list

	DisplayString *m_stringList;  ///< list of all display strings
	DisplayString *m_currentCheckpoint; ///< current checkpoint of strings to be freed
};

// EXTERNALS //////////////////////////////////////////////////////////////////////////////////////
extern DisplayStringManager *TheDisplayStringManager;  ///< singleton extern

#endif // __DISPLAYSTRINGMANAGER_H_


// FILE: DisplayStringManager.cpp /////////////////////////////////////////////////////////////////
// Created:    Colin Day, July 2001
// Desc:       Access for creating game managed display strings
///////////////////////////////////////////////////////////////////////////////////////////////////

#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "GameClient/DisplayStringManager.h"

// PUBLIC DATA ////////////////////////////////////////////////////////////////////////////////////
DisplayStringManager *TheDisplayStringManager = NULL;

///////////////////////////////////////////////////////////////////////////////////////////////////
// PUBLIC FUNCTIONS
///////////////////////////////////////////////////////////////////////////////////////////////////

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
DisplayStringManager::DisplayStringManager( void )
{

	m_stringList = NULL;
	m_currentCheckpoint = NULL;

}  // end DisplayStringManager

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
DisplayStringManager::~DisplayStringManager( void )
{

	//
	// we only keep track of the strings, we do NOT de-allocate them, our
	// list better be cleaned out before we destroy ourselves
	//
	assert( m_stringList == NULL );

}  // end ~DisplayStringManager

//-------------------------------------------------------------------------------------------------
/** Link a display string to the master list */
//-------------------------------------------------------------------------------------------------
void DisplayStringManager::link( DisplayString *string )
{

	assert( string );
	assert( string->m_next == NULL );
	assert( string->m_prev == NULL );

	string->m_next = m_stringList;
	if( m_stringList )
		m_stringList->m_prev = string;

	m_stringList = string;

}  // end link

//-------------------------------------------------------------------------------------------------
/** Unlink a display string from the master list */
//-------------------------------------------------------------------------------------------------
void DisplayStringManager::unLink( DisplayString *string )
{

	assert( string );
	assert( m_stringList );

	if( string->m_next )
		string->m_next->m_prev = string->m_prev;
	if( string->m_prev )
		string->m_prev->m_next = string->m_next;
	else
	{

		assert( string == m_stringList );
		m_stringList = string->m_next;

	}  // end else

}  // end unLink

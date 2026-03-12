// FILE: StatsCollector.h /////////////////////////////////////////////////
//-----------------------------------------------------------------------------
//                                                                          
//                       Electronic Arts Pacific.                          
//                                                                          
//                       Confidential Information                           
//                Copyright (C) 2002 - All Rights Reserved                  
//                                                                          
//-----------------------------------------------------------------------------
//
//	created:	Jul 2002
//
//	Filename: 	StatsCollector.h
//
//	author:		Chris Huybregts
//	
//	purpose:	Convinience class to help with collecting stats.
//
//-----------------------------------------------------------------------------
///////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __STATSCOLLECTOR_H_
#define __STATSCOLLECTOR_H_

//-----------------------------------------------------------------------------
// SYSTEM INCLUDES ////////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------

//-----------------------------------------------------------------------------
// USER INCLUDES //////////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------

//-----------------------------------------------------------------------------
// FORWARD REFERENCES /////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------
class GameMessage;


//-----------------------------------------------------------------------------
// TYPE DEFINES ///////////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------
class StatsCollector
{
public:
	StatsCollector( void );
	~StatsCollector( void );
	
	void reset( void );							///< Reset's all values and writes the file header
	
	void collectMsgStats( const GameMessage *msg );			///< collects Msg Stats if
	void collectUnitCountStats( void );									///< cycle through all units and takes count
	void incrementScrollMoveCount( void );
	void incrementBuildCount( void );
	void incrementAttackCount( void );
	void incrementMoveCount( void );
	void startScrollTime( void );		///< Start our logging on the amount of time we're scrolling
	void endScrollTime( void );			///< end our logging on the amount of time we're scrolling

	void update( void );						///< called once a frame to see if we should poll this frame
	
	void writeFileEnd(void);				///< Write the end of the file
private:
	
	void createFileName( void );		///< Create a snazzy filename
	AsciiString m_statsFileName;		///< store the snazzy filename
	
	void writeInitialFileInfo(void );		///< write the header file info
	void writeStatInfo( void );					///< write the stats we're keeping track of

	void zeroOutStats( void );			///< zero out the stats
	UnsignedInt m_buildCommands;		///< count of the build commands the local player issued
	UnsignedInt m_moveCommands;			///< count of the move commands
	UnsignedInt m_attackCommands;		///< attack commands
	UnsignedInt m_scrollMapCommands;///< scroll map commands
	UnsignedInt m_AIUnits;					///< tally of all the AI Units
	UnsignedInt m_playerUnits;			///< tally of all the player Units
	
	UnsignedInt m_scrollBeginTime;	///< Begin time in frames
	UnsignedInt m_scrollTime;				///< our totals for the scrolltime
	Bool m_isScrolling;							///< flag to make sure we are scrolling

	Int m_timeCount;								///< the current timeframe we're on
	Int m_lastUpdate;								///< last time we updated
	Int m_startFrame;								///< frame we started on
};


//-----------------------------------------------------------------------------
// INLINING ///////////////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------

//-----------------------------------------------------------------------------
// EXTERNALS //////////////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------
extern StatsCollector* TheStatsCollector;			///< we need a singleton

#endif // __STATSCOLLECTOR_H_

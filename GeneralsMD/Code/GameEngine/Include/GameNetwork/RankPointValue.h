// FILE: RankPointValue.h /////////////////////////////////////////////////
//-----------------------------------------------------------------------------
//                                                                          
//                       Electronic Arts Pacific.                          
//                                                                          
//                       Confidential Information                           
//                Copyright (C) 2002 - All Rights Reserved                  
//                                                                          
//-----------------------------------------------------------------------------
//
//	created:	Sep 2002
//
//	Filename: 	RankPointValue.h
//
//	author:		Chris Huybregts
//	
//	purpose:	
//
//-----------------------------------------------------------------------------
///////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __RANK_POINT_VALUE_H_
#define __RANK_POINT_VALUE_H_

//-----------------------------------------------------------------------------
// SYSTEM INCLUDES ////////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------

//-----------------------------------------------------------------------------
// USER INCLUDES //////////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------

//-----------------------------------------------------------------------------
// FORWARD REFERENCES /////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------
class Image;
class PSPlayerStats;
//-----------------------------------------------------------------------------
// TYPE DEFINES ///////////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------
enum
{
	RANK_PRIVATE = 0,
	RANK_CORPORAL,
	RANK_SERGEANT,
	RANK_LIEUTENANT,
	RANK_CAPTAIN,
	RANK_MAJOR,
	RANK_COLONEL,
	RANK_BRIGADIER_GENERAL,
	RANK_GENERAL,
	RANK_COMMANDER_IN_CHIEF,

	MAX_RANKS // keep last
};

struct RankPoints
{
RankPoints(void );
	Int m_ranks[MAX_RANKS];
	Real m_winMultiplier;
	Real m_lostMultiplier;
	Real m_hourSpentOnlineMultiplier;
	Real m_completedSoloCampaigns;
	Real m_disconnectMultiplier;
};
//-----------------------------------------------------------------------------
// INLINING ///////////////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------

//-----------------------------------------------------------------------------
// EXTERNALS //////////////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------
Int CalculateRank( const PSPlayerStats& stats );
Int GetFavoriteSide( const PSPlayerStats& stats );
const Image* LookupSmallRankImage(Int side, Int rankPoints);
extern RankPoints *TheRankPointValues;

#endif // __RANK_POINT_VALUE_H_

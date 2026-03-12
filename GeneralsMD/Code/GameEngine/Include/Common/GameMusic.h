//
// Project:    RTS 3
//
// File name:  Common/GameMusic.h
//
// Created:    5/01/01
//
//----------------------------------------------------------------------------

#pragma once

#ifndef __COMMON_GAMEMUSIC_H_
#define __COMMON_GAMEMUSIC_H_


//----------------------------------------------------------------------------
//           Includes                                                      
//----------------------------------------------------------------------------

#include "Common/GameAudio.h"
#include "Common/GameMemory.h"


//----------------------------------------------------------------------------
//           Forward References
//----------------------------------------------------------------------------

class AudioEventRTS;
struct FieldParse;

//----------------------------------------------------------------------------
//           Type Defines
//----------------------------------------------------------------------------


//===============================
// MusicTrack
//===============================

//-------------------------------------------------------------------------------------------------
/** The MusicTrack struct holds all information about a music track. 
	* Place data in TrackInfo that is useful to the game code in determining 
	* what tracks to play. */
//-------------------------------------------------------------------------------------------------

class MusicTrack : public MemoryPoolObject
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( MusicTrack, "MusicTrack" )

public:

		MusicTrack();
		// virtual destructor prototype defined by memory pool object

		const FieldParse *getFieldParse( void ) const { return m_musicTrackFieldParseTable; }
			
		Int					index;									///< Track index
		AsciiString name;										///< Logical name of track
		AsciiString filename;								///< Filename with extension of music track
		Real				volume;									///< Mixing level for this track
		Bool				ambient;								///< Game info about this track(public)

		MusicTrack *next;
		MusicTrack *prev;

	static const FieldParse m_musicTrackFieldParseTable[];		///< the parse table for INI definition

};

class MusicManager
{
	public:
		MusicManager();
		virtual ~MusicManager();

		void playTrack( AudioEventRTS *eventToUse );
		void stopTrack( AudioHandle eventToRemove );

		virtual void addAudioEvent(AudioEventRTS *eventToAdd);	// pre-copied
		virtual void removeAudioEvent( AudioHandle eventToRemove );

		void setVolume( Real m_volume );
};

#endif // __COMMON_GAMEMUSIC_H_

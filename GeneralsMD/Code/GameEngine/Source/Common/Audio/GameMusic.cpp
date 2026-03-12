//
// Project:   RTS3
//
// File name: GameMusic.cpp
//
// Created:   5/01/01
//
//----------------------------------------------------------------------------

//----------------------------------------------------------------------------
//         Includes                                                      
//----------------------------------------------------------------------------

#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "Common/GameMusic.h"

#include "Common/AudioEventRTS.h"
#include "Common/AudioRequest.h"
#include "Common/GameAudio.h"
#include "Common/INI.h"

#ifdef _INTERNAL
//#pragma optimize("", off)
//#pragma MESSAGE("************************************** WARNING, optimization disabled for debugging purposes")
#endif

//----------------------------------------------------------------------------
//         Externals                                                     
//----------------------------------------------------------------------------



//----------------------------------------------------------------------------
//         Defines                                                         
//----------------------------------------------------------------------------
#define MUSIC_PATH "Data\\Audio\\Tracks"  // directory path to the music files


//----------------------------------------------------------------------------
//         Private Types                                                     
//----------------------------------------------------------------------------

//-------------------------------------------------------------------------------------------------
/** The INI data fields for music tracks */
//-------------------------------------------------------------------------------------------------
const FieldParse MusicTrack::m_musicTrackFieldParseTable[] = 
{

	{ "Filename",								INI::parseAsciiString,							NULL, offsetof( MusicTrack, filename ) },
	{ "Volume",									INI::parsePercentToReal,						NULL, offsetof( MusicTrack, volume ) },
	{ "Ambient",								INI::parseBool,											NULL, offsetof( MusicTrack, ambient ) },
	{ NULL,											NULL,																NULL, 0 },
};


//-------------------------------------------------------------------------------------------------
MusicManager::MusicManager()
{	

}

//-------------------------------------------------------------------------------------------------
MusicManager::~MusicManager()
{

}

//-------------------------------------------------------------------------------------------------
void MusicManager::playTrack( AudioEventRTS *eventToUse )
{
	AudioRequest *audioRequest = TheAudio->allocateAudioRequest( true );
	audioRequest->m_pendingEvent = eventToUse;
	audioRequest->m_request = AR_Play;
	TheAudio->appendAudioRequest( audioRequest );
}

//-------------------------------------------------------------------------------------------------
void MusicManager::stopTrack( AudioHandle eventToRemove )
{
	AudioRequest *audioRequest = TheAudio->allocateAudioRequest( false );
	audioRequest->m_handleToInteractOn = eventToRemove;
	audioRequest->m_request = AR_Stop;
	TheAudio->appendAudioRequest( audioRequest );
}

//-------------------------------------------------------------------------------------------------
void MusicManager::addAudioEvent( AudioEventRTS *eventToAdd )
{
	playTrack( eventToAdd );
}

//-------------------------------------------------------------------------------------------------
void MusicManager::removeAudioEvent( AudioHandle eventToRemove )
{
	stopTrack( eventToRemove );
}


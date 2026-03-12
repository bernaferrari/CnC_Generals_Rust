// FILE: version.h //////////////////////////////////////////////////////
// Generals version number class
// Author: Matthew D. Campbell, November 2001

#pragma once

#ifndef __VERSION_H__
#define __VERSION_H__

/** 
 * The Version class formats the version number into integer and string
 * values for different parts of the game.
 * @todo: increment build number on compile, and stamp exe with username
 */
class Version
{
public:
	Version();
	UnsignedInt getVersionNumber( void );						///< Return a 4-byte integer suitable for WOLAPI
	AsciiString getAsciiVersion( void );						///< Return a human-readable version number
	UnicodeString getUnicodeVersion( void );				///< Return a human-readable version number
	UnicodeString getFullUnicodeVersion( void );		///< Return a human-readable version number
	AsciiString getAsciiBuildTime( void );					///< Return a formated date/time string for build time
	UnicodeString getUnicodeBuildTime( void );			///< Return a formated date/time string for build time
	AsciiString getAsciiBuildLocation( void );			///< Return a string with the build location
	UnicodeString getUnicodeBuildLocation( void );	///< Return a string with the build location
	AsciiString getAsciiBuildUser( void );					///< Return a string with the build user
	UnicodeString getUnicodeBuildUser( void );			///< Return a string with the build user

	Bool showFullVersion( void ) { return m_showFullVersion; }
	void setShowFullVersion( Bool val ) { m_showFullVersion = val; }

	void setVersion(Int major, Int minor, Int buildNum,
		Int localBuildNum, AsciiString user, AsciiString location,
		AsciiString buildTime, AsciiString buildDate); ///< Set version info

private:
	Int m_major;
	Int m_minor;
	Int m_buildNum;
	Int m_localBuildNum;
	AsciiString m_buildLocation;
	AsciiString m_buildUser;
	AsciiString m_buildTime;
	AsciiString m_buildDate;
	Bool m_showFullVersion;
};

extern Version *TheVersion;	///< The Version singleton

#endif // __VERSION_H__

//
// Project:    WSYS Library
//
// Module:		String
//
// File name:  wsys/string.h
//
// Created:    11/02/01
//
//----------------------------------------------------------------------------

#pragma once

#ifndef __WSYS_STRING_H
#define __WSYS_STRING_H


//----------------------------------------------------------------------------
//           Includes                                                      
//----------------------------------------------------------------------------

#include "Lib/BaseType.h"

//----------------------------------------------------------------------------
//           Forward References
//----------------------------------------------------------------------------



//----------------------------------------------------------------------------
//           Type Defines
//----------------------------------------------------------------------------


class WSYS_String
{
	protected:

		Char	*m_data;										///< actual string data

	public:

	explicit WSYS_String(const Char *string = NULL);
	~WSYS_String();

	// operators
	Bool								operator== (const char *rvalue) const;
	Bool								operator!= (const char *rvalue) const;
	const WSYS_String&	operator= (const WSYS_String &string);
	const WSYS_String&	operator= (const Char *string);
	const WSYS_String&	operator+= (const WSYS_String &string);
	const WSYS_String&	operator+= (const Char *string);
	friend WSYS_String	operator+ (const WSYS_String &string1, const WSYS_String &string2);
	friend WSYS_String	operator+ (const Char *string1, const WSYS_String &string2);
	friend WSYS_String	operator+ (const WSYS_String &string1, const Char *string2);
	const Char &				operator[] (Int index) const;
	Char &							operator[] (Int index);
											operator const Char * (void) const;
											operator Char * (void) const ;

	// methods
	void								makeUpperCase( void );
	void								makeLowerCase( void );
	Int									length(void) const;
	Bool								isEmpty(void) const;
	Int _cdecl					format(const Char *format, ...);
	void								set( const Char *string );
	Char*								get( void ) const;
};



//----------------------------------------------------------------------------
//           Inlining                                                       
//----------------------------------------------------------------------------

inline Char* WSYS_String::get( void ) const { return m_data;};
inline const Char& WSYS_String::operator[] (Int index) const{ return m_data[index];};
inline Char& WSYS_String::operator[] (Int index) { return m_data[index];};
inline WSYS_String::operator const Char * (void) const { return m_data;};
inline WSYS_String::operator Char * (void) const {return m_data;};


#endif // __WSYS_STRING_H

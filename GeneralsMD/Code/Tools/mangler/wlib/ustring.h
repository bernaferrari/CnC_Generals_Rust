#ifndef USTRING_HEADER
#define USTRING_HEADER

#include <stdlib.h>
#include <stdio.h>
#include <iostream.h>
#include <string>

// Windows headers have a tendency to redefine IN
#ifdef IN
#undef IN
#endif
#define IN const

#define MAX_BYTES_PER_CHAR 1

template <class charT>
class UstringT : public basic_string<charT, string_char_traits<charT> >
{
 public:
		explicit UstringT(int max_charlength) {
			set_max_bytelength(max_charlength*MAX_BYTES_PER_CHAR);
		}

		UstringT() { max_bytelength=4000; }

      size_t   get_max_bytelength(void) { return(max_bytelength); }
      void     set_max_bytelength(size_t max) { max_bytelength=max; }

      bool     operator==(const UstringT<charT> &other)
      {
        const basic_string<charT, string_char_traits<charT> > *other_basic=&other;
        const basic_string<charT, string_char_traits<charT> > *this_basic=this;
        return((*other_basic)==(*this_basic));
      }

 private:
		size_t   max_bytelength;
};

typedef UstringT<char> Ustring;

#endif

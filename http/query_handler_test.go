package http

import (
	"bytes"
	"context"
	"fmt"
	"net/http"
	"net/http/httptest"
	"reflect"
	"regexp"
	"testing"

	"github.com/influxdata/flux/csv"
	"github.com/influxdata/flux/lang"
	"github.com/influxdata/platform/query"
)

func TestFluxService_Query(t *testing.T) {
	tests := []struct {
		name    string
		token   string
		ctx     context.Context
		r       *query.ProxyRequest
		status  int
		want    int64
		wantW   string
		wantErr bool
	}{
		{
			name:  "query",
			ctx:   context.Background(),
			token: "mytoken",
			r: &query.ProxyRequest{
				Request: query.Request{
					Compiler: lang.FluxCompiler{
						Query: "from()",
					},
				},
				Dialect: csv.DefaultDialect(),
			},
			status: http.StatusOK,
			want:   6,
			wantW:  "howdy\n",
		},
		{
			name:  "error status",
			token: "mytoken",
			ctx:   context.Background(),
			r: &query.ProxyRequest{
				Request: query.Request{
					Compiler: lang.FluxCompiler{
						Query: "from()",
					},
				},
				Dialect: csv.DefaultDialect(),
			},
			status:  http.StatusUnauthorized,
			wantErr: true,
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
				w.WriteHeader(tt.status)
				fmt.Fprintln(w, "howdy")
			}))
			defer ts.Close()
			s := &FluxService{
				URL:   ts.URL,
				Token: tt.token,
			}

			w := &bytes.Buffer{}
			got, err := s.Query(tt.ctx, w, tt.r)
			if (err != nil) != tt.wantErr {
				t.Errorf("FluxService.Query() error = %v, wantErr %v", err, tt.wantErr)
				return
			}
			if got != tt.want {
				t.Errorf("FluxService.Query() = %v, want %v", got, tt.want)
			}
			if gotW := w.String(); gotW != tt.wantW {
				t.Errorf("FluxService.Query() = %v, want %v", gotW, tt.wantW)
			}
		})
	}
}

func TestFluxQueryService_Query(t *testing.T) {
	tests := []struct {
		name    string
		token   string
		ctx     context.Context
		r       *query.Request
		csv     string
		status  int
		want    string
		wantErr bool
	}{
		{
			name:  "error status",
			token: "mytoken",
			ctx:   context.Background(),
			r: &query.Request{
				Compiler: lang.FluxCompiler{
					Query: "from()",
				},
			},
			status:  http.StatusUnauthorized,
			wantErr: true,
		},
		{
			name:  "returns csv",
			token: "mytoken",
			ctx:   context.Background(),
			r: &query.Request{
				Compiler: lang.FluxCompiler{
					Query: "from()",
				},
			},
			status: http.StatusOK,
			csv: `#datatype,string,long,dateTime:RFC3339,double,long,string,boolean,string,string,string
#group,false,false,false,false,false,false,false,true,true,true
#default,0,,,,,,,,,
,result,table,_time,usage_user,test,mystr,this,cpu,host,_measurement
,,0,2018-08-29T13:08:47Z,10.2,10,yay,true,cpu-total,a,cpui
`,
			want: toCRLF(`,,,2018-08-29T13:08:47Z,10.2,10,yay,true,cpu-total,a,cpui

`),
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
				w.WriteHeader(tt.status)
				fmt.Fprintln(w, tt.csv)
			}))
			s := &FluxQueryService{
				URL:   ts.URL,
				Token: tt.token,
			}
			res, err := s.Query(tt.ctx, tt.r)
			if (err != nil) != tt.wantErr {
				t.Errorf("FluxQueryService.Query() error = %v, wantErr %v", err, tt.wantErr)
				return
			}
			if res != nil && res.Err() != nil {
				t.Errorf("FluxQueryService.Query() result error = %v", res.Err())
				return
			}
			if tt.wantErr {
				return
			}
			defer res.Cancel()

			enc := csv.NewMultiResultEncoder(csv.ResultEncoderConfig{
				NoHeader:  true,
				Delimiter: ',',
			})
			b := bytes.Buffer{}
			n, err := enc.Encode(&b, res)
			if err != nil {
				t.Errorf("FluxQueryService.Query() encode error = %v", err)
				return
			}
			if n != int64(len(tt.want)) {
				t.Errorf("FluxQueryService.Query() encode result = %d, want %d", n, len(tt.want))
			}

			got := b.String()
			if !reflect.DeepEqual(got, tt.want) {
				t.Errorf("FluxQueryService.Query() =\n%s\n%s", got, tt.want)
			}
		})
	}
}

var crlfPattern = regexp.MustCompile(`\r?\n`)

func toCRLF(data string) string {
	return crlfPattern.ReplaceAllString(data, "\r\n")
}
